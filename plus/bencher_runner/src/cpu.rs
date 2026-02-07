//! CPU isolation for benchmark runners.
//!
//! This module provides CPU core partitioning and thread affinity management
//! to isolate benchmark execution from housekeeping tasks (heartbeat, networking).
//!
//! # Layout
//!
//! On startup, the runner detects available CPUs and partitions them:
//! - **Housekeeping cores**: Core 0 (and core 1 on systems with 8+ cores)
//! - **Benchmark cores**: All remaining cores
//!
//! The heartbeat thread is pinned to housekeeping cores, while the Firecracker
//! VM process is pinned to benchmark cores via cgroups cpuset.

#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::io;

/// CPU layout for the runner.
///
/// Partitions available cores into housekeeping (for heartbeat, networking)
/// and benchmark (for Firecracker VM) sets.
#[derive(Debug, Clone)]
pub struct CpuLayout {
    /// Cores reserved for housekeeping tasks (heartbeat, networking, etc.)
    pub housekeeping: Vec<usize>,
    /// Cores reserved for benchmark execution (Firecracker VM)
    pub benchmark: Vec<usize>,
}

impl CpuLayout {
    /// Detect available CPUs and create a layout.
    ///
    /// Reserves 1-2 cores for housekeeping (depending on total core count)
    /// and assigns the rest to benchmarks.
    #[must_use]
    pub fn detect() -> Self {
        let num_cores = Self::available_cores();
        Self::with_core_count(num_cores)
    }

    /// Create a layout for a given number of cores.
    ///
    /// - 1 core: housekeeping = [0], benchmark = [0] (shared, no isolation)
    /// - 2-7 cores: housekeeping = [0], benchmark = [1..n]
    /// - 8+ cores: housekeeping = [0, 1], benchmark = [2..n]
    #[must_use]
    pub fn with_core_count(num_cores: usize) -> Self {
        if num_cores == 0 {
            // Fallback for edge cases
            return Self {
                housekeeping: vec![0],
                benchmark: vec![0],
            };
        }

        if num_cores == 1 {
            // Single core - no isolation possible
            return Self {
                housekeeping: vec![0],
                benchmark: vec![0],
            };
        }

        // Reserve 1 core for housekeeping on small systems, 2 on larger ones
        let housekeeping_count = if num_cores >= 8 { 2 } else { 1 };

        let housekeeping: Vec<usize> = (0..housekeeping_count).collect();
        let benchmark: Vec<usize> = (housekeeping_count..num_cores).collect();

        Self {
            housekeeping,
            benchmark,
        }
    }

    /// Get the number of available CPU cores.
    #[cfg(target_os = "linux")]
    fn available_cores() -> usize {
        // Try reading from /sys/devices/system/cpu/online first
        if let Ok(online) = fs::read_to_string("/sys/devices/system/cpu/online") {
            if let Some(count) = parse_cpu_list(&online) {
                return count;
            }
        }

        // Fallback to nix sysconf
        match nix::unistd::sysconf(nix::unistd::SysconfVar::_SC_NPROCESSORS_ONLN) {
            Ok(Some(n)) if n > 0 => n as usize,
            _ => 1, // Ultimate fallback
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn available_cores() -> usize {
        std::thread::available_parallelism()
            .map(std::num::NonZero::get)
            .unwrap_or(1)
    }

    /// Format benchmark cores as a cpuset string (e.g., "2-7" or "2,3,4,5").
    #[must_use]
    pub fn benchmark_cpuset(&self) -> String {
        format_cpuset(&self.benchmark)
    }

    /// Format housekeeping cores as a cpuset string.
    #[must_use]
    pub fn housekeeping_cpuset(&self) -> String {
        format_cpuset(&self.housekeeping)
    }

    /// Check if meaningful isolation is possible.
    ///
    /// Returns false if there's only 1 core or housekeeping/benchmark overlap.
    #[must_use]
    pub fn has_isolation(&self) -> bool {
        !self.benchmark.is_empty()
            && !self.housekeeping.is_empty()
            && self
                .housekeeping
                .iter()
                .all(|h| !self.benchmark.contains(h))
    }
}

/// Parse a CPU list string like "0-3" or "0,2,4" or "0-3,8-11".
///
/// Returns the total count of CPUs in the list.
#[cfg(target_os = "linux")]
fn parse_cpu_list(s: &str) -> Option<usize> {
    let mut count = 0;
    for part in s.trim().split(',') {
        let part = part.trim();
        if let Some((start, end)) = part.split_once('-') {
            let start: usize = start.trim().parse().ok()?;
            let end: usize = end.trim().parse().ok()?;
            count += end - start + 1;
        } else {
            let _: usize = part.parse().ok()?;
            count += 1;
        }
    }
    Some(count)
}

/// Format a list of CPU IDs as a cpuset string.
///
/// Produces compact ranges where possible (e.g., [2,3,4,5] -> "2-5").
fn format_cpuset(cpus: &[usize]) -> String {
    use std::fmt::Write as _;

    if cpus.is_empty() {
        return String::new();
    }

    let mut sorted: Vec<usize> = cpus.to_vec();
    sorted.sort_unstable();
    sorted.dedup();

    let mut result = String::new();
    let mut i = 0;

    while i < sorted.len() {
        let Some(&start) = sorted.get(i) else {
            break;
        };
        let mut end = start;

        // Find consecutive range
        while i + 1 < sorted.len() {
            let Some(&next) = sorted.get(i + 1) else {
                break;
            };
            if next != end + 1 {
                break;
            }
            i += 1;
            end = next;
        }

        if !result.is_empty() {
            result.push(',');
        }

        if start == end {
            // write! to String is infallible
            let _ = write!(result, "{start}");
        } else {
            let _ = write!(result, "{start}-{end}");
        }

        i += 1;
    }

    result
}

/// Pin the current thread to the specified CPU cores.
///
/// Uses `sched_setaffinity` on Linux via the `nix` crate.
#[cfg(target_os = "linux")]
pub fn pin_current_thread(cpus: &[usize]) -> io::Result<()> {
    use nix::sched::{CpuSet, sched_setaffinity};
    use nix::unistd::Pid;

    if cpus.is_empty() {
        return Ok(());
    }

    let mut set = CpuSet::new();
    for &cpu in cpus {
        set.set(cpu)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    }

    sched_setaffinity(Pid::from_raw(0), &set).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

/// Pin the current thread to the specified CPU cores.
///
/// No-op on non-Linux platforms.
#[cfg(not(target_os = "linux"))]
pub fn pin_current_thread(_cpus: &[usize]) -> std::io::Result<()> {
    Ok(())
}

/// Pin a thread by its native handle to the specified CPU cores.
///
/// Uses `pthread_setaffinity_np` on Linux.
#[cfg(target_os = "linux")]
pub fn pin_thread(thread_id: libc::pthread_t, cpus: &[usize]) -> io::Result<()> {
    if cpus.is_empty() {
        return Ok(());
    }

    // SAFETY: Same as pin_current_thread, plus we're passing a valid pthread_t.
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_ZERO(&mut set);

        for &cpu in cpus {
            if cpu < libc::CPU_SETSIZE as usize {
                libc::CPU_SET(cpu, &mut set);
            }
        }

        let result =
            libc::pthread_setaffinity_np(thread_id, std::mem::size_of::<libc::cpu_set_t>(), &set);

        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(result))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_single_core() {
        let layout = CpuLayout::with_core_count(1);
        assert_eq!(layout.housekeeping, vec![0]);
        assert_eq!(layout.benchmark, vec![0]);
        assert!(!layout.has_isolation());
    }

    #[test]
    fn layout_two_cores() {
        let layout = CpuLayout::with_core_count(2);
        assert_eq!(layout.housekeeping, vec![0]);
        assert_eq!(layout.benchmark, vec![1]);
        assert!(layout.has_isolation());
    }

    #[test]
    fn layout_four_cores() {
        let layout = CpuLayout::with_core_count(4);
        assert_eq!(layout.housekeeping, vec![0]);
        assert_eq!(layout.benchmark, vec![1, 2, 3]);
        assert!(layout.has_isolation());
    }

    #[test]
    fn layout_eight_cores() {
        let layout = CpuLayout::with_core_count(8);
        assert_eq!(layout.housekeeping, vec![0, 1]);
        assert_eq!(layout.benchmark, vec![2, 3, 4, 5, 6, 7]);
        assert!(layout.has_isolation());
    }

    #[test]
    fn layout_sixteen_cores() {
        let layout = CpuLayout::with_core_count(16);
        assert_eq!(layout.housekeeping, vec![0, 1]);
        assert_eq!(layout.benchmark.len(), 14);
        assert!(layout.has_isolation());
    }

    #[test]
    fn format_cpuset_empty() {
        assert_eq!(format_cpuset(&[]), "");
    }

    #[test]
    fn format_cpuset_single() {
        assert_eq!(format_cpuset(&[3]), "3");
    }

    #[test]
    fn format_cpuset_range() {
        assert_eq!(format_cpuset(&[2, 3, 4, 5]), "2-5");
    }

    #[test]
    fn format_cpuset_mixed() {
        assert_eq!(format_cpuset(&[0, 2, 3, 4, 7, 8]), "0,2-4,7-8");
    }

    #[test]
    fn format_cpuset_unsorted() {
        assert_eq!(format_cpuset(&[5, 3, 4, 2]), "2-5");
    }

    #[test]
    fn benchmark_cpuset_string() {
        let layout = CpuLayout::with_core_count(8);
        assert_eq!(layout.benchmark_cpuset(), "2-7");
    }

    #[test]
    fn housekeeping_cpuset_string() {
        let layout = CpuLayout::with_core_count(8);
        assert_eq!(layout.housekeeping_cpuset(), "0-1");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_cpu_list_simple() {
        assert_eq!(parse_cpu_list("0-3"), Some(4));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_cpu_list_complex() {
        assert_eq!(parse_cpu_list("0-3,8-11"), Some(8));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_cpu_list_individual() {
        assert_eq!(parse_cpu_list("0,2,4"), Some(3));
    }
}
