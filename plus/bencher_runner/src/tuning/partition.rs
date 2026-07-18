//! Cpuset scheduler partition for the parent `bencher` cgroup.
//!
//! Turning `/sys/fs/cgroup/bencher` into an `isolated` cpuset partition
//! removes the benchmark cores from the root scheduling domain: the kernel
//! load balancer can no longer pull other tasks onto them. This is the
//! runtime equivalent of the `isolcpus=` boot argument. Per-run cgroups
//! created under the partition inherit it automatically.

#![expect(
    clippy::print_stderr,
    reason = "partition setup prints best-effort warnings"
)]

use std::fs;

use camino::{Utf8Path, Utf8PathBuf};

use super::TuningGuard;
use crate::cpu::CpuLayout;
use crate::error::JailError;
use crate::jail::{BENCHER_CGROUP_BASE, effective_mems};

/// Manages the parent `bencher` cgroup as a cpuset scheduler partition.
///
/// The partition must be a child of a valid partition root; the cgroup
/// root always qualifies, which is why this lives on the parent `bencher`
/// cgroup and not on a per-run child.
pub(super) struct BencherPartition {
    /// The parent `bencher` cgroup path.
    path: Utf8PathBuf,
    /// The cgroup v2 mount point.
    root: Utf8PathBuf,
}

/// Achieved cpuset partition level, in decreasing order of isolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PartitionLevel {
    /// Scheduler domain isolation: no load balancing onto benchmark cores.
    Isolated,
    /// A separate scheduler domain with load balancing inside it.
    Root,
    /// A plain cgroup (no partition); cpuset pinning still applies.
    Member,
}

impl BencherPartition {
    /// Create a partition manager for the given cgroup v2 mount point
    /// (`/sys/fs/cgroup` in production; tests pass a tempdir tree).
    #[must_use]
    pub(super) fn new(cgroup_root: &Utf8Path) -> Self {
        Self {
            path: cgroup_root.join(BENCHER_CGROUP_BASE),
            root: cgroup_root.to_owned(),
        }
    }

    /// Turn the `bencher` cgroup into a cpuset partition on the benchmark
    /// cores, verifying the kernel accepted it.
    ///
    /// Falls back `isolated` -> `root` -> `member` when the kernel reports
    /// the partition as invalid (read-back verification). All writes are
    /// recorded on the guard and restored in reverse order on drop.
    /// Best-effort: any failure degrades to [`PartitionLevel::Member`],
    /// which matches the behavior before partitions were introduced.
    pub(super) fn apply(&self, layout: &CpuLayout, guard: &mut TuningGuard) -> PartitionLevel {
        // The bencher cgroup only gets cpuset files once the cpuset
        // controller is enabled in the root subtree. Additive and
        // idempotent, so it is not saved for restore.
        if let Err(e) = fs::write(self.root.join("cgroup.subtree_control"), "+cpuset") {
            eprintln!("Warning: failed to enable cpuset controller in cgroup root: {e}");
            return PartitionLevel::Member;
        }

        if !self.path.exists()
            && let Err(e) = fs::create_dir_all(&self.path)
        {
            eprintln!("Warning: failed to create cgroup {}: {e}", self.path);
            return PartitionLevel::Member;
        }

        // A partition needs explicit cpus and mems. Mems mirror the
        // root's effective nodes so multi-node NUMA hosts are not forced
        // onto node 0.
        if !save_and_write(
            guard,
            &self.path.join("cpuset.cpus"),
            &layout.benchmark_cpuset(),
            "bencher cpuset.cpus",
        ) {
            return PartitionLevel::Member;
        }
        if !save_and_write(
            guard,
            &self.path.join("cpuset.mems"),
            &effective_mems(&self.root),
            "bencher cpuset.mems",
        ) {
            return PartitionLevel::Member;
        }

        let partition_path = self.path.join("cpuset.cpus.partition");
        let original = match fs::read_to_string(&partition_path) {
            Ok(value) => partition_mode_token(&value).to_owned(),
            Err(e) => {
                // Missing on kernels without cpuset partition support.
                eprintln!("Warning: cpuset partitions unavailable ({partition_path}: {e})");
                return PartitionLevel::Member;
            },
        };

        // Save the restore entry before attempting any mode write: a write
        // can succeed at the syscall level while the kernel rejects the
        // partition in the read-back, and the file must still revert on
        // drop. Restoring an unchanged value is a harmless no-op.
        guard.save_restore(
            partition_path.clone(),
            original,
            "bencher cpuset partition".to_owned(),
        );

        for (mode, level) in [
            ("isolated", PartitionLevel::Isolated),
            ("root", PartitionLevel::Root),
        ] {
            match try_partition_mode(&partition_path, mode) {
                Ok(()) => return level,
                Err(e) => {
                    eprintln!("Warning: cpuset partition mode '{mode}' not achieved: {e}");
                },
            }
        }

        PartitionLevel::Member
    }
}

impl std::fmt::Display for PartitionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Isolated => write!(f, "isolated"),
            Self::Root => write!(f, "root"),
            Self::Member => write!(f, "member"),
        }
    }
}

/// Write a partition mode and verify the kernel accepted it.
///
/// The kernel reports a rejected partition in the read-back value
/// (e.g., `isolated invalid (Cpu list in cpuset.cpus not exclusive)`),
/// so a successful write is not enough.
fn try_partition_mode(path: &Utf8Path, mode: &str) -> Result<(), JailError> {
    fs::write(path, mode).map_err(|e| JailError::WriteCgroup {
        path: path.to_owned(),
        source: e,
    })?;

    verify_partition_state(path, mode)
}

/// Extract the mode token from a `cpuset.cpus.partition` read-back.
///
/// A healthy partition reads back as just the mode (`member`, `root`,
/// `isolated`); a rejected one as `<mode> invalid (<reason>)`. Only the
/// mode token is a valid value to write back on restore. An empty or
/// unreadable value falls back to the kernel default `member`.
fn partition_mode_token(state: &str) -> &str {
    state.split_whitespace().next().unwrap_or("member")
}

/// Read back a partition file and check the kernel reports exactly `mode`.
fn verify_partition_state(path: &Utf8Path, mode: &str) -> Result<(), JailError> {
    let state = fs::read_to_string(path).map_err(|e| JailError::WriteCgroup {
        path: path.to_owned(),
        source: e,
    })?;
    let state = state.trim();

    if state == mode {
        Ok(())
    } else {
        Err(JailError::PartitionInvalid {
            mode: mode.to_owned(),
            state: state.to_owned(),
        })
    }
}

/// Save the current value of a cgroup file on the guard, then write `value`.
///
/// An empty current value is saved as a newline so the restore write is
/// not a zero-byte no-op (clearing `cpuset.cpus` requires writing `"\n"`).
/// Returns false (with a warning) when the file cannot be read or written.
fn save_and_write(guard: &mut TuningGuard, path: &Utf8Path, value: &str, label: &str) -> bool {
    let current = match fs::read_to_string(path) {
        Ok(current) => current.trim().to_owned(),
        Err(e) => {
            eprintln!("Warning: failed to read {path}: {e}");
            return false;
        },
    };

    if current == value {
        return true;
    }

    if let Err(e) = fs::write(path, value) {
        eprintln!("Warning: failed to write {path}: {e}");
        return false;
    }

    let restore_value = if current.is_empty() {
        "\n".to_owned()
    } else {
        current
    };
    guard.save_restore(path.to_owned(), restore_value, label.to_owned());
    true
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use crate::cpu::CpuLayout;
    use crate::tuning::{TuningConfig, TuningGuard};

    use super::*;

    fn empty_guard() -> TuningGuard {
        crate::tuning::apply(&TuningConfig::disabled())
    }

    /// A fake cgroup v2 tree mirroring what the kernel exposes.
    fn fake_cgroup_root() -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        fs::write(root.join("cgroup.subtree_control"), "").unwrap();
        fs::create_dir_all(root.join("bencher")).unwrap();
        fs::write(root.join("bencher/cpuset.cpus"), "").unwrap();
        fs::write(root.join("bencher/cpuset.mems"), "").unwrap();
        fs::write(root.join("bencher/cpuset.cpus.partition"), "member\n").unwrap();

        (dir, root)
    }

    #[test]
    fn partition_applies_isolated() {
        let (_dir, root) = fake_cgroup_root();
        let layout = CpuLayout::with_core_count(8);
        let mut guard = empty_guard();

        let level = BencherPartition::new(&root).apply(&layout, &mut guard);

        assert_eq!(level, PartitionLevel::Isolated);
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.cpus")).unwrap(),
            "2-7"
        );
        // No cpuset.mems.effective in the fake root: node 0 fallback.
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.mems")).unwrap(),
            "0"
        );
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.cpus.partition")).unwrap(),
            "isolated"
        );
    }

    #[test]
    fn partition_mems_mirror_root_effective() {
        let (_dir, root) = fake_cgroup_root();
        fs::write(root.join("cpuset.mems.effective"), "0-1\n").unwrap();
        let layout = CpuLayout::with_core_count(8);
        let mut guard = empty_guard();

        let level = BencherPartition::new(&root).apply(&layout, &mut guard);

        assert_eq!(level, PartitionLevel::Isolated);
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.mems")).unwrap(),
            "0-1"
        );
    }

    #[test]
    fn partition_restores_on_guard_drop() {
        let (_dir, root) = fake_cgroup_root();
        let layout = CpuLayout::with_core_count(8);

        {
            let mut guard = empty_guard();
            let level = BencherPartition::new(&root).apply(&layout, &mut guard);
            assert_eq!(level, PartitionLevel::Isolated);
        }

        // Partition demoted back, cpus and mems cleared (newline write).
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.cpus.partition")).unwrap(),
            "member"
        );
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.cpus")).unwrap(),
            "\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.mems")).unwrap(),
            "\n"
        );
    }

    #[test]
    fn partition_falls_back_to_member_when_unwritable() {
        let (_dir, root) = fake_cgroup_root();
        // A directory in place of the partition file forces every mode
        // write to fail, exercising the full fallback chain.
        fs::remove_file(root.join("bencher/cpuset.cpus.partition")).unwrap();
        fs::create_dir_all(root.join("bencher/cpuset.cpus.partition")).unwrap();
        let layout = CpuLayout::with_core_count(8);
        let mut guard = empty_guard();

        let level = BencherPartition::new(&root).apply(&layout, &mut guard);

        assert_eq!(level, PartitionLevel::Member);
    }

    #[test]
    fn partition_member_without_cgroup_v2() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let layout = CpuLayout::with_core_count(8);
        let mut guard = empty_guard();

        let level = BencherPartition::new(&root).apply(&layout, &mut guard);

        assert_eq!(level, PartitionLevel::Member);
    }

    #[test]
    fn partition_restore_entry_saved_before_mode_writes() {
        let (_dir, root) = fake_cgroup_root();
        // A previous run (or crash) left the partition file dirty: the
        // kernel reports rejected partitions inline in the read-back.
        fs::write(
            root.join("bencher/cpuset.cpus.partition"),
            "isolated invalid (Cpu list not exclusive)\n",
        )
        .unwrap();
        let layout = CpuLayout::with_core_count(8);

        {
            let mut guard = empty_guard();
            let level = BencherPartition::new(&root).apply(&layout, &mut guard);
            assert_eq!(level, PartitionLevel::Isolated);
        }

        // The restore writes back the normalized mode token, never the
        // full dirty state (which is not a valid value to write).
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.cpus.partition")).unwrap(),
            "isolated"
        );
    }

    #[test]
    fn partition_mode_token_normalizes_states() {
        assert_eq!(partition_mode_token("member\n"), "member");
        assert_eq!(partition_mode_token("isolated\n"), "isolated");
        assert_eq!(
            partition_mode_token("isolated invalid (Cpu list not exclusive)\n"),
            "isolated"
        );
        assert_eq!(partition_mode_token(""), "member");
    }

    #[test]
    fn verify_partition_state_accepts_exact_mode() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let path = root.join("cpuset.cpus.partition");
        fs::write(&path, "isolated\n").unwrap();

        verify_partition_state(&path, "isolated").unwrap();
    }

    #[test]
    fn verify_partition_state_rejects_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let path = root.join("cpuset.cpus.partition");
        // The kernel reports a rejected partition in the read-back value.
        fs::write(&path, "isolated invalid (Cpu list not exclusive)\n").unwrap();

        let err = verify_partition_state(&path, "isolated").unwrap_err();
        assert!(err.to_string().contains("invalid"));
    }

    #[test]
    fn partition_level_display() {
        assert_eq!(PartitionLevel::Isolated.to_string(), "isolated");
        assert_eq!(PartitionLevel::Root.to_string(), "root");
        assert_eq!(PartitionLevel::Member.to_string(), "member");
    }
}
