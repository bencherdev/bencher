//! Cpuset scheduler partition for the parent `bencher` cgroup.
//!
//! Turning `/sys/fs/cgroup/bencher` into an `isolated` cpuset partition
//! removes the benchmark cores from the root scheduling domain: the kernel
//! load balancer can no longer pull other tasks onto them. This is the
//! runtime equivalent of the `isolcpus=` boot argument. Per-run cgroups
//! created under the partition inherit it automatically.

use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use slog::{Logger, warn};

use crate::cpu::CpuLayout;
use crate::error::JailError;
use crate::jail::{BENCHER_CGROUP_BASE, Controllers, effective_mems, ensure_controllers};

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
    /// the partition as invalid (read-back verification). Nothing is reverted,
    /// so the partition outlives the runner until the host reboots.
    /// Best-effort: any failure degrades to [`PartitionLevel::Member`],
    /// which matches the behavior before partitions were introduced.
    pub(super) fn apply(&self, log: &Logger, layout: &CpuLayout) -> PartitionLevel {
        self.apply_with(log, layout, |root| ensure_controllers(log, root))
    }

    /// The controllers step is injectable because only cgroupfs gives its
    /// writes their meaning.
    fn apply_with<E>(&self, log: &Logger, layout: &CpuLayout, ensure: E) -> PartitionLevel
    where
        E: FnOnce(&Utf8Path) -> Result<Controllers, JailError>,
    {
        if let Err(e) = fs::create_dir_all(&self.path) {
            warn!(log, "Cgroup not created"; "cgroup" => self.path.as_str(), "error" => %e);
            return PartitionLevel::Member;
        }

        // The bencher cgroup has cpuset files only once the root enables cpuset.
        if !report_controllers(log, ensure(&self.root)) {
            return PartitionLevel::Member;
        }

        // A partition needs explicit cpus and mems. Mems mirror the
        // root's effective nodes so multi-node NUMA hosts are not forced
        // onto node 0.
        if !write_unless_set(
            log,
            &self.path.join("cpuset.cpus"),
            &layout.benchmark_cpuset(),
        ) {
            return PartitionLevel::Member;
        }
        // An unreadable node set degrades to member rather than guessing one,
        // which would confine the benchmark's memory on a read that never happened.
        let mems = match effective_mems(&self.root) {
            Ok(mems) => mems,
            Err(e) => {
                warn!(log, "No cpuset partition, the cgroup root's memory nodes are unreadable";
                    "error" => %e,
                );
                return PartitionLevel::Member;
            },
        };
        if !write_unless_set(log, &self.path.join("cpuset.mems"), &mems) {
            return PartitionLevel::Member;
        }

        let partition_path = self.partition_path();
        if let Err(e) = fs::read_to_string(&partition_path) {
            // Missing on kernels without cpuset partition support.
            warn!(log, "Cpuset partitions unavailable"; "path" => partition_path.as_str(), "error" => %e);
            return PartitionLevel::Member;
        }

        for (mode, level) in [
            ("isolated", PartitionLevel::Isolated),
            ("root", PartitionLevel::Root),
        ] {
            match try_partition_mode(&partition_path, mode) {
                Ok(()) => return level,
                Err(e) => {
                    warn!(log, "Cpuset partition mode not achieved"; "mode" => mode, "error" => %e);
                },
            }
        }

        // A refused mode reads back as `<mode> invalid (...)` until replaced.
        if let Err(e) = fs::write(&partition_path, "member") {
            warn!(log, "Cpuset partition mode not written back"; "mode" => "member", "error" => %e);
        }
        PartitionLevel::Member
    }

    pub(super) fn partition_path(&self) -> Utf8PathBuf {
        self.path.join("cpuset.cpus.partition")
    }
}

/// Warns for each controller Jobs will lack, naming it and the cgroup that
/// withheld it, and returns whether they get a cpuset.
fn report_controllers(log: &Logger, controllers: Result<Controllers, JailError>) -> bool {
    let controllers = match controllers {
        Ok(controllers) => controllers,
        Err(e) => {
            warn!(log, "Cgroup controllers Jobs use not enabled"; "error" => %e);
            return false;
        },
    };
    if let Some(absence) = controllers.memory_absence() {
        warn!(log, "Jobs will have no swap limit"; "reason" => %absence);
    }
    if let Some(absence) = controllers.cpuset_absence() {
        warn!(log, "Jobs will have no cgroup cpuset"; "reason" => %absence);
        return false;
    }
    true
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

/// Write `value` to a cgroup file unless it already holds it.
///
/// Returns false (with a warning) when the file cannot be read or written.
fn write_unless_set(log: &Logger, path: &Utf8Path, value: &str) -> bool {
    let current = match fs::read_to_string(path) {
        Ok(current) => current.trim().to_owned(),
        Err(e) => {
            warn!(log, "Cgroup file unreadable"; "path" => path.as_str(), "error" => %e);
            return false;
        },
    };

    if current == value {
        return true;
    }

    if let Err(e) = fs::write(path, value) {
        warn!(log, "Cgroup file not written"; "path" => path.as_str(), "error" => %e);
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use crate::cpu::CpuLayout;
    use crate::log::discard;

    use super::*;

    /// Only cgroupfs gives the controller writes their meaning, so a tempfile
    /// tree takes them as done.
    fn apply_on_tree(root: &Utf8Path, layout: &CpuLayout) -> PartitionLevel {
        BencherPartition::new(root).apply_with(&discard(), layout, |_| Ok(Controllers::enabled()))
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
        let level = apply_on_tree(&root, &layout);

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
        let level = apply_on_tree(&root, &layout);

        assert_eq!(level, PartitionLevel::Isolated);
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.mems")).unwrap(),
            "0-1"
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
        let level = apply_on_tree(&root, &layout);

        assert_eq!(level, PartitionLevel::Member);
    }

    #[test]
    fn partition_member_without_cgroup_v2() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let layout = CpuLayout::with_core_count(8);
        let level = BencherPartition::new(&root).apply(&discard(), &layout);

        assert_eq!(level, PartitionLevel::Member);
    }

    #[test]
    fn a_restart_finds_the_partition_in_place() {
        // Kills dropping the equality check: the kernel prints a newline the
        // runner never writes, so a rewrite loses it.
        let (_dir, root) = fake_cgroup_root();
        fs::write(root.join("bencher/cpuset.cpus"), "2-7\n").unwrap();
        fs::write(root.join("bencher/cpuset.mems"), "0\n").unwrap();
        fs::write(root.join("bencher/cpuset.cpus.partition"), "isolated\n").unwrap();
        let layout = CpuLayout::with_core_count(8);

        let level = apply_on_tree(&root, &layout);

        assert_eq!(level, PartitionLevel::Isolated);
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.cpus")).unwrap(),
            "2-7\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.mems")).unwrap(),
            "0\n"
        );
    }

    #[test]
    fn a_partition_left_invalid_is_applied_again() {
        // Kills trusting the mode token alone: a refused partition reads back
        // as `<mode> invalid (<reason>)`.
        let (_dir, root) = fake_cgroup_root();
        fs::write(
            root.join("bencher/cpuset.cpus.partition"),
            "isolated invalid (Cpu list not exclusive)\n",
        )
        .unwrap();
        let layout = CpuLayout::with_core_count(8);

        let level = apply_on_tree(&root, &layout);

        assert_eq!(level, PartitionLevel::Isolated);
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.cpus.partition")).unwrap(),
            "isolated"
        );
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

    #[test]
    fn without_cpuset_there_is_no_partition_to_make() {
        // Going on would fail on the missing `cpuset.cpus` and blame the file.
        let (_dir, root) = fake_cgroup_root();
        let layout = CpuLayout::with_core_count(8);
        let level = BencherPartition::new(&root).apply_with(&discard(), &layout, |root| {
            Ok(Controllers::without_cpuset(root))
        });

        assert_eq!(level, PartitionLevel::Member);
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.cpus")).unwrap(),
            "",
            "no partition was attempted"
        );
    }

    #[test]
    #[expect(clippy::print_stderr, reason = "a skipped test says why")]
    fn a_partition_mode_the_kernel_refuses_is_written_back_to_member() {
        // Otherwise the file reads `root invalid (...)` for the life of the
        // runner. A cgroup whose parent is not a partition root admits neither
        // mode.
        if crate::jail::current_euid() != 0 {
            eprintln!(
                "skipped a_partition_mode_the_kernel_refuses_is_written_back_to_member: building a cgroup needs root"
            );
            return;
        }
        let layout = CpuLayout::detect(&discard());
        if !layout.has_isolation() {
            eprintln!(
                "skipped a_partition_mode_the_kernel_refuses_is_written_back_to_member: this host has no core to partition"
            );
            return;
        }
        let scratch = crate::jail::ScratchCgroup::new("bencher-runner-partition");

        let level = BencherPartition::new(scratch.path()).apply(&discard(), &layout);
        let partition =
            fs::read_to_string(scratch.path().join("bencher/cpuset.cpus.partition")).unwrap();

        assert_eq!(level, PartitionLevel::Member);
        assert_eq!(partition.trim(), "member");
    }
}
