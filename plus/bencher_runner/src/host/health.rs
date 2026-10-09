//! The host's disk health: md at every probe, `NVMe` at most hourly.

use std::io;
use std::time::{Duration, Instant};

use bencher_json::runner::{
    HealthFindingKind, HealthState, JsonHealthFinding, JsonNvmeHealth, JsonRunnerHealth,
};
use camino::Utf8Path;
use slog::{Logger, warn};

use super::md::MdArray;
use super::nvme::{self, SMART_LOG_LEN};

/// How long an `NVMe` read stands, since each is an admin command to the drive.
const NVME_INTERVAL: Duration = Duration::from_hours(1);
const DEV: &str = "/dev";

/// The last `NVMe` read, kept between probes.
#[derive(Default)]
pub(crate) struct Health {
    nvme: Option<(Instant, Vec<JsonNvmeHealth>)>,
    warned: bool,
}

impl Health {
    pub(crate) fn read(
        &mut self,
        log: &Logger,
        sysfs: &Utf8Path,
        arrays: &[MdArray],
    ) -> JsonRunnerHealth {
        self.read_with(
            log,
            sysfs,
            Utf8Path::new(DEV),
            arrays,
            Instant::now(),
            nvme::read_log,
        )
    }

    fn read_with<F>(
        &mut self,
        log: &Logger,
        sysfs: &Utf8Path,
        dev: &Utf8Path,
        arrays: &[MdArray],
        now: Instant,
        read_log: F,
    ) -> JsonRunnerHealth
    where
        F: Fn(&Utf8Path) -> io::Result<[u8; SMART_LOG_LEN]>,
    {
        let stale = self
            .nvme
            .as_ref()
            .is_none_or(|(read, _)| now.saturating_duration_since(*read) >= NVME_INTERVAL);
        if stale {
            let controllers = nvme::controllers(sysfs, dev, read_log);
            if !self.warned
                && let Some(error) = controllers
                    .iter()
                    .find_map(|controller| controller.error.as_ref())
            {
                warn!(log, "NVMe health not read, so it is reported unreadable"; "error" => %error);
                self.warned = true;
            }
            self.nvme = Some((
                now,
                controllers
                    .into_iter()
                    .map(|controller| controller.health)
                    .collect(),
            ));
        }
        let nvme = self
            .nvme
            .as_ref()
            .map(|(_, nvme)| nvme.clone())
            .unwrap_or_default();
        health(arrays, nvme)
    }
}

/// The worst finding sets the state, and a degraded array is failing.
fn health(arrays: &[MdArray], nvme: Vec<JsonNvmeHealth>) -> JsonRunnerHealth {
    let mut findings: Vec<JsonHealthFinding> = arrays
        .iter()
        .filter(|array| array.degraded.is_some_and(|missing| missing > 0))
        .map(|array| JsonHealthFinding {
            device: array.name.clone(),
            kind: HealthFindingKind::Degraded,
            state: HealthState::Failing,
        })
        .collect();
    for controller in &nvme {
        if let Some(log) = &controller.log {
            findings.extend(nvme::findings(&controller.name, log));
        }
    }
    let state = findings
        .iter()
        .map(|finding| finding.state.clone())
        .max()
        .unwrap_or(HealthState::Ok);
    JsonRunnerHealth {
        state,
        findings,
        arrays: arrays.iter().map(MdArray::json).collect(),
        nvme,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use bencher_json::runner::{JsonMdArray, MdSyncAction};
    use camino::Utf8PathBuf;

    use super::*;
    use crate::host::md;
    use crate::host::nvme::tests::page;
    use crate::log::discard;

    /// A sysfs root with one `NVMe` controller beside `arrays`, each array's
    /// `degraded` and `array_state` given.
    fn sysfs(arrays: &[(&str, &str, u32)]) -> (tempfile::TempDir, Utf8PathBuf) {
        let synced: Vec<_> = arrays
            .iter()
            .map(|(name, _, _)| (*name, "idle", "none", "none"))
            .collect();
        let (dir, root) = md::tests::sysfs(&synced);
        for (name, state, degraded) in arrays {
            let md = root.join("block").join(name).join("md");
            std::fs::write(md.join("array_state"), format!("{state}\n")).unwrap();
            std::fs::write(md.join("degraded"), format!("{degraded}\n")).unwrap();
        }
        std::fs::create_dir_all(root.join("class/nvme/nvme0")).unwrap();
        (dir, root)
    }

    #[test]
    fn a_degraded_array_is_failing() {
        // Kills a degraded array that reads healthy, or one whose members are
        // all present that reads failing.
        let (_dir, root) = sysfs(&[("md0", "clean", 0), ("md1", "active", 1)]);
        let arrays = md::arrays(&root);
        let health = Health::default().read_with(
            &discard(),
            &root,
            &root.join("dev"),
            &arrays,
            Instant::now(),
            |_| Ok(page(&[])),
        );
        assert_eq!(health.state, HealthState::Failing);
        assert_eq!(
            health.findings,
            [JsonHealthFinding {
                device: "md1".to_owned(),
                kind: HealthFindingKind::Degraded,
                state: HealthState::Failing,
            }]
        );
        assert_eq!(
            health.arrays.get(1),
            Some(&JsonMdArray {
                name: "md1".to_owned(),
                array_state: "active".to_owned(),
                degraded: 1,
                action: MdSyncAction::Idle,
            })
        );
    }

    #[test]
    fn the_worst_finding_sets_the_state() {
        // Kills a state taken from the first finding, or left `ok` beside a
        // warning.
        let (_dir, root) = sysfs(&[]);
        let read = |page: [u8; SMART_LOG_LEN]| {
            Health::default()
                .read_with(
                    &discard(),
                    &root,
                    &root.join("dev"),
                    &[],
                    Instant::now(),
                    |_| Ok(page),
                )
                .state
        };
        assert_eq!(read(page(&[])), HealthState::Ok);
        assert_eq!(read(page(&[(5, 90)])), HealthState::Warning);
        // The temperature warning comes before the spare below its threshold.
        assert_eq!(read(page(&[(0, 1 << 1), (3, 5)])), HealthState::Failing);
    }

    #[test]
    fn nvme_is_read_at_most_hourly() {
        // Kills a read on every probe, an admin command to the drive each
        // poll, and a read that is never repeated.
        let (_dir, root) = sysfs(&[]);
        let reads = Cell::new(0);
        let mut health = Health::default();
        let start = Instant::now();
        let mut read_at = |elapsed| {
            health.read_with(
                &discard(),
                &root,
                &root.join("dev"),
                &[],
                start + elapsed,
                |_| {
                    reads.set(reads.get() + 1);
                    Ok(page(&[(5, 90)]))
                },
            )
        };

        assert_eq!(read_at(Duration::ZERO).state, HealthState::Warning);
        let cached = read_at(NVME_INTERVAL.checked_sub(Duration::from_secs(1)).unwrap());
        assert_eq!(reads.get(), 1);
        assert_eq!(cached.state, HealthState::Warning, "the cache is reported");
        read_at(NVME_INTERVAL);
        assert_eq!(reads.get(), 2);
    }

    #[test]
    fn an_unreadable_nvme_still_reports_md() {
        // Kills a runner that is not root reporting nothing, rather than md
        // and its controllers as unreadable.
        let (_dir, root) = sysfs(&[("md0", "clean", 1)]);
        let arrays = md::arrays(&root);
        let health = Health::default().read_with(
            &discard(),
            &root,
            &root.join("dev"),
            &arrays,
            Instant::now(),
            |_| Err(io::ErrorKind::PermissionDenied.into()),
        );
        assert_eq!(health.state, HealthState::Failing);
        assert_eq!(health.arrays.len(), 1);
        assert_eq!(health.nvme.len(), 1);
        assert_eq!(health.nvme.first().and_then(|nvme| nvme.log.as_ref()), None);
    }
}
