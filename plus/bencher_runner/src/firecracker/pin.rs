//! Pin Firecracker threads to dedicated benchmark cores.
//!
//! Inside an isolated cpuset partition there is no load balancing, and
//! even without one, vCPU threads migrating between benchmark cores adds
//! run-to-run variance (cold caches, TLB refills). Firecracker names its
//! vCPU threads `fc_vcpu {index}`; each is pinned to its own benchmark
//! core, and the remaining VMM threads are pinned to the last benchmark
//! core (housekeeping cores are outside the VM cgroup cpuset, so pinning
//! there would fail with EINVAL). Each pin is best-effort with warnings.

use camino::Utf8Path;
use slog::{Logger, info, warn};

use crate::cpu::{CpuLayout, pin_tid};

/// A thread of the VMM: its id and its name.
pub(super) type Task = (libc::pid_t, String);

/// Pin the threads the VMM had once its vCPU threads started.
///
/// The guest kernel runs on its vCPUs for the moment between their start and
/// this, confined from birth to the benchmark cores by the cgroup cpuset;
/// per-thread pinning is a refinement on top that stops migration between them.
pub(super) fn pin_vcpu_threads(log: &Logger, tasks: &[Task], layout: &CpuLayout, vcpu_count: u8) {
    let (_, pinned_vcpus) = pin_threads(log, tasks, layout);
    info!(log, "vCPU threads pinned to dedicated cores";
        "pinned" => pinned_vcpus,
        "vcpus" => vcpu_count,
    );
}

/// Pin every thread the VMM has now, which reaches one started since the
/// first pass, as KVM's NX recovery worker is once vCPU 0 first runs.
pub(super) fn pin_threads_again(log: &Logger, tasks: &[Task], layout: &CpuLayout) {
    let (pinned, _) = pin_threads(log, tasks, layout);
    info!(log, "VMM threads pinned again";
        "pinned" => pinned,
        "threads" => tasks.len(),
    );
}

/// Returns how many threads, and how many vCPU threads, were pinned.
fn pin_threads(log: &Logger, tasks: &[Task], layout: &CpuLayout) -> (usize, usize) {
    let mut pinned = 0usize;
    let mut pinned_vcpus = 0usize;
    for (tid, comm) in tasks {
        let Some(core) = assign_core(comm, &layout.benchmark) else {
            continue;
        };
        match pin_tid(*tid, &[core]) {
            Ok(()) => {
                pinned += 1;
                if parse_vcpu_comm(comm).is_some() {
                    pinned_vcpus += 1;
                }
            },
            Err(e) => {
                warn!(log, "Firecracker thread not pinned";
                    "thread" => bencher_logger::capped(comm),
                    "tid" => *tid,
                    "core" => core,
                    "error" => %e,
                );
            },
        }
    }
    (pinned, pinned_vcpus)
}

/// How many of `tasks` are vCPU threads.
pub(super) fn vcpus_in(tasks: &[Task]) -> usize {
    tasks
        .iter()
        .filter(|(_, comm)| parse_vcpu_comm(comm).is_some())
        .count()
}

/// Read all (tid, comm) pairs under a `/proc/<pid>/task` directory.
///
/// Threads that exit mid-scan are silently skipped.
pub(super) fn read_tasks(task_dir: &Utf8Path) -> Vec<Task> {
    let Ok(entries) = std::fs::read_dir(task_dir.as_std_path()) else {
        return Vec::new();
    };

    let mut tasks = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(tid) = name.to_str().and_then(|name| name.parse().ok()) else {
            continue;
        };
        let Ok(comm) = std::fs::read_to_string(entry.path().join("comm")) else {
            continue;
        };
        tasks.push((tid, comm.trim().to_owned()));
    }

    tasks
}

/// Parse a Firecracker vCPU thread name (`fc_vcpu {index}`) into its index.
fn parse_vcpu_comm(comm: &str) -> Option<usize> {
    comm.trim().strip_prefix("fc_vcpu ")?.parse().ok()
}

/// Choose the benchmark core for a Firecracker thread.
///
/// vCPU `N` gets its own core (`benchmark[N % len]`); all other threads
/// share the last benchmark core, keeping them off the cores running the
/// earlier vCPUs. When the vCPU count equals the benchmark core count, the
/// highest-index vCPU therefore shares its core with the VMM threads and
/// sees more scheduling noise than its peers; the lower-index vCPUs keep
/// dedicated cores.
fn assign_core(comm: &str, benchmark: &[usize]) -> Option<usize> {
    let &last = benchmark.last()?;
    match parse_vcpu_comm(comm) {
        Some(index) => benchmark.get(index % benchmark.len()).copied(),
        None => Some(last),
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use super::*;

    #[test]
    fn parses_vcpu_comm() {
        assert_eq!(parse_vcpu_comm("fc_vcpu 0"), Some(0));
        assert_eq!(parse_vcpu_comm("fc_vcpu 12"), Some(12));
        assert_eq!(parse_vcpu_comm("fc_vcpu 3\n"), Some(3));
    }

    #[test]
    fn rejects_non_vcpu_comm() {
        assert_eq!(parse_vcpu_comm("firecracker"), None);
        assert_eq!(parse_vcpu_comm("fc_vcpu"), None);
        assert_eq!(parse_vcpu_comm("fc_vcpu x"), None);
        assert_eq!(parse_vcpu_comm(""), None);
    }

    #[test]
    fn assigns_vcpus_to_dedicated_cores() {
        let benchmark = vec![2, 3, 4, 5];
        assert_eq!(assign_core("fc_vcpu 0", &benchmark), Some(2));
        assert_eq!(assign_core("fc_vcpu 1", &benchmark), Some(3));
        assert_eq!(assign_core("fc_vcpu 3", &benchmark), Some(5));
    }

    #[test]
    fn wraps_vcpus_beyond_core_count() {
        let benchmark = vec![2, 3];
        assert_eq!(assign_core("fc_vcpu 2", &benchmark), Some(2));
        assert_eq!(assign_core("fc_vcpu 3", &benchmark), Some(3));
    }

    #[test]
    fn assigns_vmm_threads_to_last_core() {
        let benchmark = vec![2, 3, 4, 5];
        assert_eq!(assign_core("firecracker", &benchmark), Some(5));
    }

    #[test]
    fn no_benchmark_cores_assigns_nothing() {
        assert_eq!(assign_core("fc_vcpu 0", &[]), None);
        assert_eq!(assign_core("firecracker", &[]), None);
    }

    #[test]
    fn reads_tasks_from_fake_proc_tree() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        std::fs::create_dir_all(root.join("100")).unwrap();
        std::fs::create_dir_all(root.join("101")).unwrap();
        std::fs::write(root.join("100/comm"), "firecracker\n").unwrap();
        std::fs::write(root.join("101/comm"), "fc_vcpu 0\n").unwrap();

        let mut tasks = read_tasks(&root);
        tasks.sort_unstable();
        assert_eq!(
            tasks,
            vec![
                (100, "firecracker".to_owned()),
                (101, "fc_vcpu 0".to_owned())
            ]
        );
    }

    #[test]
    fn read_tasks_missing_dir_is_empty() {
        assert_eq!(read_tasks(Utf8Path::new("/nonexistent/task")), Vec::new());
    }
}
