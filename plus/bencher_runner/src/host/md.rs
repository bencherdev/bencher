//! md RAID arrays, read from sysfs.

use bencher_json::runner::{JsonMdArray, MdSyncAction, PauseReason};
use camino::Utf8Path;

/// An md array's sync state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MdArray {
    pub name: String,
    /// `None` for a personality with no sync thread, such as raid0.
    pub action: Option<MdSyncAction>,
    pub completed: SyncCompleted,
    /// KiB per second.
    pub speed: Option<u64>,
    /// Missing members.
    pub degraded: Option<u32>,
    pub array_state: Option<String>,
}

/// An array's `sync_completed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyncCompleted {
    /// No sync thread is running.
    None,
    /// A sync thread is queued behind another array's.
    Delayed,
    Progress {
        done: u64,
        total: u64,
    },
    Unreadable,
}

/// Every md array under `sysfs`, by name.
pub(crate) fn arrays(sysfs: &Utf8Path) -> Vec<MdArray> {
    let Ok(entries) = sysfs.join("block").read_dir_utf8() else {
        return Vec::new();
    };
    let mut arrays: Vec<MdArray> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().starts_with("md"))
        .filter_map(|entry| MdArray::read(entry.file_name(), &entry.path().join("md")))
        .collect();
    arrays.sort_by(|a, b| a.name.cmp(&b.name));
    arrays
}

/// A pause reason for each array a sync thread is running on.
pub(crate) fn raid_reasons(arrays: &[MdArray]) -> Vec<PauseReason> {
    arrays
        .iter()
        .filter(|array| array.busy())
        .map(MdArray::pause_reason)
        .collect()
}

impl MdArray {
    fn read(name: &str, md: &Utf8Path) -> Option<Self> {
        if !md.is_dir() {
            return None;
        }
        let read = |attribute: &str| {
            std::fs::read_to_string(md.join(attribute))
                .ok()
                .map(|value| value.trim().to_owned())
        };
        let completed = match read("sync_completed").as_deref() {
            None | Some("none") => SyncCompleted::None,
            Some("delayed") => SyncCompleted::Delayed,
            Some(progress) => parse_progress(progress).unwrap_or(SyncCompleted::Unreadable),
        };
        Some(Self {
            name: name.to_owned(),
            action: read("sync_action").map(|action| parse_action(&action)),
            completed,
            speed: read("sync_speed").and_then(|speed| speed.parse().ok()),
            degraded: read("degraded").and_then(|degraded| degraded.parse().ok()),
            array_state: read("array_state"),
        })
    }

    /// An array with no sync thread, such as raid0, reads as idle.
    pub(crate) fn json(&self) -> JsonMdArray {
        JsonMdArray {
            name: self.name.clone(),
            array_state: self.array_state.clone().unwrap_or_default(),
            degraded: self.degraded.unwrap_or_default(),
            action: self.action.clone().unwrap_or(MdSyncAction::Idle),
        }
    }

    /// `sync_action` alone can read `recover` while no recovery can run, so
    /// a sync thread must also be running or queued.
    pub(crate) fn busy(&self) -> bool {
        matches!(
            self.action,
            Some(
                MdSyncAction::Resync
                    | MdSyncAction::Recover
                    | MdSyncAction::Check
                    | MdSyncAction::Repair
                    | MdSyncAction::Reshape
            )
        ) && self.completed != SyncCompleted::None
    }

    fn pause_reason(&self) -> PauseReason {
        let (done, total) = if let SyncCompleted::Progress { done, total } = self.completed {
            (Some(done), Some(total))
        } else {
            (None, None)
        };
        PauseReason::Raid {
            array: self.name.clone(),
            action: self
                .action
                .clone()
                .unwrap_or_else(|| MdSyncAction::Other(String::new())),
            done,
            total,
            speed: self.speed,
        }
    }
}

fn parse_action(action: &str) -> MdSyncAction {
    match action {
        "resync" => MdSyncAction::Resync,
        "recover" => MdSyncAction::Recover,
        "check" => MdSyncAction::Check,
        "repair" => MdSyncAction::Repair,
        "reshape" => MdSyncAction::Reshape,
        "frozen" => MdSyncAction::Frozen,
        "idle" => MdSyncAction::Idle,
        other => MdSyncAction::Other(other.to_owned()),
    }
}

fn parse_progress(progress: &str) -> Option<SyncCompleted> {
    let (done, total) = progress.split_once('/')?;
    Some(SyncCompleted::Progress {
        done: done.trim().parse().ok()?,
        total: total.trim().parse().ok()?,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use camino::Utf8PathBuf;

    use super::*;

    /// A sysfs root holding one md array per `(name, sync_action, sync_completed, sync_speed)`.
    pub(crate) fn sysfs(arrays: &[(&str, &str, &str, &str)]) -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_owned()).unwrap();
        std::fs::create_dir_all(root.join("block/sda/queue")).unwrap();
        for (name, action, completed, speed) in arrays {
            let md = root.join("block").join(name).join("md");
            std::fs::create_dir_all(&md).unwrap();
            std::fs::write(md.join("sync_action"), format!("{action}\n")).unwrap();
            std::fs::write(md.join("sync_completed"), format!("{completed}\n")).unwrap();
            std::fs::write(md.join("sync_speed"), format!("{speed}\n")).unwrap();
        }
        (dir, root)
    }

    #[test]
    fn a_running_sync_of_each_kind_pauses() {
        // Kills a busy set that misses an action, or counts frozen or idle.
        for (action, busy) in [
            ("resync", true),
            ("recover", true),
            ("check", true),
            ("repair", true),
            ("reshape", true),
            ("frozen", false),
            ("idle", false),
            ("scrubbing", false),
        ] {
            let (_dir, root) = sysfs(&[("md0", action, "10 / 100", "2048")]);
            assert_eq!(
                raid_reasons(&arrays(&root)).len(),
                usize::from(busy),
                "{action}"
            );
        }
    }

    #[test]
    fn a_sync_action_with_no_sync_thread_does_not_pause() {
        // Kills a test on `sync_action` alone, which reads `recover` while a
        // recovery is needed but cannot run.
        let (_dir, root) = sysfs(&[("md0", "recover", "none", "none")]);
        assert_eq!(raid_reasons(&arrays(&root)), []);
    }

    #[test]
    fn a_queued_sync_pauses_with_no_progress() {
        // Kills a busy test that wants progress numbers, which a queued sync
        // has none of.
        let (_dir, root) = sysfs(&[("md1", "resync", "delayed", "none")]);
        assert_eq!(
            raid_reasons(&arrays(&root)),
            [PauseReason::Raid {
                array: "md1".to_owned(),
                action: MdSyncAction::Resync,
                done: None,
                total: None,
                speed: None,
            }]
        );
    }

    #[test]
    fn a_pause_reason_carries_the_progress_and_speed() {
        // Kills swapped or dropped progress and speed fields, and an array
        // named after anything but its block device.
        let (_dir, root) = sysfs(&[
            ("md0", "idle", "none", "none"),
            ("md127", "check", "1024 / 4096", "51200"),
        ]);
        assert_eq!(
            raid_reasons(&arrays(&root)),
            [PauseReason::Raid {
                array: "md127".to_owned(),
                action: MdSyncAction::Check,
                done: Some(1024),
                total: Some(4096),
                speed: Some(51200),
            }]
        );
    }

    #[test]
    fn a_host_with_no_arrays_never_pauses() {
        // Kills a reader that takes a disk with no `md` directory for an
        // array, or fails on a host with no `block` directory.
        let (_dir, root) = sysfs(&[]);
        std::fs::create_dir_all(root.join("block/md0")).unwrap();
        assert_eq!(arrays(&root), []);
        assert_eq!(arrays(&root.join("absent")), []);
    }

    #[test]
    fn an_array_with_no_sync_thread_is_listed_but_never_busy() {
        // Kills a reader that drops a raid0 array, which has no
        // `sync_action`, or calls it busy.
        let (_dir, root) = sysfs(&[]);
        std::fs::create_dir_all(root.join("block/md2/md")).unwrap();
        let arrays = arrays(&root);
        assert_eq!(
            arrays,
            [MdArray {
                name: "md2".to_owned(),
                action: None,
                completed: SyncCompleted::None,
                speed: None,
                degraded: None,
                array_state: None,
            }]
        );
        assert_eq!(raid_reasons(&arrays), []);
    }
}
