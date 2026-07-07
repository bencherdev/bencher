//! Pin Firecracker threads to dedicated benchmark cores.
//!
//! Inside an isolated cpuset partition there is no load balancing, and
//! even without one, vCPU threads migrating between benchmark cores adds
//! run-to-run variance (cold caches, TLB refills). Firecracker names its
//! vCPU threads `fc_vcpu {index}`; each is pinned to its own benchmark
//! core, and the remaining VMM/API threads are pinned to the last
//! benchmark core (housekeeping cores are outside the VM cgroup cpuset,
//! so pinning there would fail with EINVAL). All of this is best-effort
//! with warnings.

use std::time::{Duration, Instant};

use camino::{Utf8Path, Utf8PathBuf};

use crate::cpu::{CpuLayout, pin_tid};

/// How long to wait for all vCPU threads to appear after `InstanceStart`.
const DISCOVERY_TIMEOUT: Duration = Duration::from_millis(500);

/// Poll interval while waiting for vCPU threads.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Pin the Firecracker process's threads to dedicated benchmark cores.
///
/// Polls `/proc/<pid>/task/*/comm` until `vcpu_count` vCPU threads appear
/// (they are spawned during `InstanceStart`) or the discovery timeout
/// elapses, then pins whatever was found.
///
/// vCPU threads necessarily run for a moment before being pinned (they
/// do not exist until `InstanceStart`). The cgroup cpuset is the hard
/// confinement to benchmark cores; per-thread pinning is a refinement
/// on top of it that stops migration between those cores.
pub(super) fn pin_vcpu_threads(fc_pid: u32, layout: &CpuLayout, vcpu_count: u8) {
    let task_dir = Utf8PathBuf::from(format!("/proc/{fc_pid}/task"));
    let deadline = Instant::now() + DISCOVERY_TIMEOUT;

    let tasks = loop {
        let tasks = read_tasks(&task_dir);
        let vcpus_found = tasks
            .iter()
            .filter(|(_, comm)| parse_vcpu_comm(comm).is_some())
            .count();
        if vcpus_found >= usize::from(vcpu_count) || Instant::now() >= deadline {
            break tasks;
        }
        std::thread::sleep(POLL_INTERVAL);
    };

    if tasks.is_empty() {
        eprintln!("Warning: found no Firecracker threads to pin under {task_dir}");
        return;
    }

    let mut pinned_vcpus = 0usize;
    for (tid, comm) in &tasks {
        let Some(core) = assign_core(comm, &layout.benchmark) else {
            continue;
        };
        match pin_tid(*tid, &[core]) {
            Ok(()) => {
                if parse_vcpu_comm(comm).is_some() {
                    pinned_vcpus += 1;
                }
            },
            Err(e) => {
                eprintln!("Warning: failed to pin thread '{comm}' (tid {tid}) to core {core}: {e}");
            },
        }
    }

    println!(
        "CPU isolation: pinned {pinned_vcpus} of {vcpu_count} vCPU threads to dedicated cores"
    );
}

/// Read all (tid, comm) pairs under a `/proc/<pid>/task` directory.
///
/// Threads that exit mid-scan are silently skipped.
fn read_tasks(task_dir: &Utf8Path) -> Vec<(libc::pid_t, String)> {
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
/// (VMM, API server) share the last benchmark core, keeping them off the
/// cores running the earlier vCPUs.
fn assign_core(comm: &str, benchmark: &[usize]) -> Option<usize> {
    let &last = benchmark.last()?;
    match parse_vcpu_comm(comm) {
        Some(index) => benchmark.get(index % benchmark.len()).copied(),
        None => Some(last),
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(parse_vcpu_comm("fc_api"), None);
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
        assert_eq!(assign_core("fc_api", &benchmark), Some(5));
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
        assert!(read_tasks(Utf8Path::new("/nonexistent/task")).is_empty());
    }
}
