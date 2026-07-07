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
    /// and assigns the rest to benchmarks. On Linux this reads the actual
    /// online CPU IDs rather than a count: disabling SMT on topologies with
    /// interleaved sibling numbering (common on AMD) leaves a non-contiguous
    /// online set like `0,2,4,6`, and a count-based layout would pin to
    /// offline cores.
    #[must_use]
    pub fn detect() -> Self {
        #[cfg(target_os = "linux")]
        if let Ok(online) = fs::read_to_string("/sys/devices/system/cpu/online")
            && let Some(ids) = parse_cpu_id_list(&online)
            && !ids.is_empty()
        {
            return Self::with_cpu_ids(ids);
        }

        Self::with_core_count(Self::available_cores())
    }

    /// Create a layout from an explicit list of online CPU IDs.
    ///
    /// IDs need not be contiguous. The lowest 1-2 IDs become housekeeping
    /// cores and the rest benchmark cores:
    /// - 1 ID: housekeeping and benchmark share it (no isolation)
    /// - 2-7 IDs: housekeeping = first, benchmark = rest
    /// - 8+ IDs: housekeeping = first two, benchmark = rest
    #[must_use]
    pub fn with_cpu_ids(mut ids: Vec<usize>) -> Self {
        ids.sort_unstable();
        ids.dedup();

        if ids.is_empty() {
            // Fallback for edge cases
            return Self {
                housekeeping: vec![0],
                benchmark: vec![0],
            };
        }

        if ids.len() == 1 {
            // Single core - no isolation possible
            return Self {
                housekeeping: ids.clone(),
                benchmark: ids,
            };
        }

        // Reserve 1 core for housekeeping on small systems, 2 on larger ones
        let housekeeping_count = if ids.len() >= 8 { 2 } else { 1 };
        let benchmark = ids.split_off(housekeeping_count);

        Self {
            housekeeping: ids,
            benchmark,
        }
    }

    /// Create a layout for a given number of contiguous cores `0..n`.
    #[must_use]
    pub fn with_core_count(num_cores: usize) -> Self {
        Self::with_cpu_ids((0..num_cores).collect())
    }

    /// Get the number of available CPU cores.
    ///
    /// Count-based fallback for when the online CPU ID list is unavailable.
    #[cfg(target_os = "linux")]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "sysconf returns c_long; core count always fits in usize"
    )]
    fn available_cores() -> usize {
        match nix::unistd::sysconf(nix::unistd::SysconfVar::_NPROCESSORS_ONLN) {
            Ok(Some(n)) if n > 0 => n as usize,
            _ => 1, // Ultimate fallback
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn available_cores() -> usize {
        std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
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

/// Parse a kernel CPU list string like "0-3" or "0,2,4" or "0-3,8-11"
/// into the expanded list of CPU IDs.
#[cfg(target_os = "linux")]
fn parse_cpu_id_list(s: &str) -> Option<Vec<usize>> {
    let mut ids = Vec::new();
    for part in s.trim().split(',') {
        let part = part.trim();
        if let Some((start, end)) = part.split_once('-') {
            let start: usize = start.trim().parse().ok()?;
            let end: usize = end.trim().parse().ok()?;
            if end < start {
                return None;
            }
            ids.extend(start..=end);
        } else {
            ids.push(part.parse().ok()?);
        }
    }
    Some(ids)
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
            _ = write!(result, "{start}");
        } else {
            _ = write!(result, "{start}-{end}");
        }

        i += 1;
    }

    result
}

/// Format a list of CPU IDs as a kernel hex bitmask string.
///
/// Produces the comma-separated 32-bit word format used by files like
/// `/proc/irq/default_smp_affinity` and
/// `/sys/devices/virtual/workqueue/cpumask`. Words are ordered most
/// significant first; all words after the first are zero-padded to
/// 8 hex digits (e.g., `[0, 1]` -> `"3"`, `[32]` -> `"1,00000000"`).
#[must_use]
#[expect(
    clippy::integer_division,
    reason = "dividing CPU IDs into 32-bit mask words"
)]
pub fn format_cpumask(cpus: &[usize]) -> String {
    use std::fmt::Write as _;

    let Some(&max) = cpus.iter().max() else {
        return "0".to_owned();
    };

    let mut words = vec![0u32; max / 32 + 1];
    for &cpu in cpus {
        if let Some(word) = words.get_mut(cpu / 32) {
            *word |= 1u32 << (cpu % 32);
        }
    }

    let mut result = String::new();
    for (index, word) in words.iter().rev().enumerate() {
        if index == 0 {
            // write! to String is infallible
            _ = write!(result, "{word:x}");
        } else {
            _ = write!(result, ",{word:08x}");
        }
    }

    result
}

/// Pin the current thread to the specified CPU cores.
///
/// Uses `sched_setaffinity` on Linux via the `nix` crate.
#[cfg(target_os = "linux")]
pub fn pin_current_thread(cpus: &[usize]) -> io::Result<()> {
    pin_tid(0, cpus)
}

/// Pin a thread by its kernel thread ID (TID) to the specified CPU cores.
///
/// Unlike [`pin_thread`], this works on threads of other processes
/// (e.g., Firecracker vCPU threads found under `/proc/<pid>/task`).
/// A TID of 0 targets the calling thread.
#[cfg(target_os = "linux")]
pub fn pin_tid(tid: libc::pid_t, cpus: &[usize]) -> io::Result<()> {
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

    sched_setaffinity(Pid::from_raw(tid), &set).map_err(io::Error::other)
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

    #[expect(
        unsafe_code,
        clippy::multiple_unsafe_ops_per_block,
        reason = "pthread_setaffinity_np requires unsafe FFI"
    )]
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
            libc::pthread_setaffinity_np(thread_id, size_of::<libc::cpu_set_t>(), &raw const set);

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
    fn layout_interleaved_smt_offline() {
        // SMT disabled on an interleaved-sibling topology (e.g., AMD):
        // only the even-numbered cores remain online.
        let layout = CpuLayout::with_cpu_ids(vec![0, 2, 4, 6]);
        assert_eq!(layout.housekeeping, vec![0]);
        assert_eq!(layout.benchmark, vec![2, 4, 6]);
        assert!(layout.has_isolation());
        assert_eq!(layout.benchmark_cpuset(), "2,4,6");
    }

    #[test]
    fn layout_interleaved_eight_ids() {
        let layout = CpuLayout::with_cpu_ids(vec![0, 2, 4, 6, 8, 10, 12, 14]);
        assert_eq!(layout.housekeeping, vec![0, 2]);
        assert_eq!(layout.benchmark, vec![4, 6, 8, 10, 12, 14]);
        assert!(layout.has_isolation());
    }

    #[test]
    fn layout_ids_unsorted_and_duplicated() {
        let layout = CpuLayout::with_cpu_ids(vec![6, 2, 0, 4, 2]);
        assert_eq!(layout.housekeeping, vec![0]);
        assert_eq!(layout.benchmark, vec![2, 4, 6]);
    }

    #[test]
    fn layout_single_nonzero_id() {
        let layout = CpuLayout::with_cpu_ids(vec![5]);
        assert_eq!(layout.housekeeping, vec![5]);
        assert_eq!(layout.benchmark, vec![5]);
        assert!(!layout.has_isolation());
    }

    #[test]
    fn layout_empty_ids() {
        let layout = CpuLayout::with_cpu_ids(Vec::new());
        assert_eq!(layout.housekeeping, vec![0]);
        assert_eq!(layout.benchmark, vec![0]);
        assert!(!layout.has_isolation());
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
    fn format_cpumask_empty() {
        assert_eq!(format_cpumask(&[]), "0");
    }

    #[test]
    fn format_cpumask_single() {
        assert_eq!(format_cpumask(&[0]), "1");
    }

    #[test]
    fn format_cpumask_low_cores() {
        assert_eq!(format_cpumask(&[0, 1]), "3");
    }

    #[test]
    fn format_cpumask_full_byte() {
        assert_eq!(format_cpumask(&[0, 1, 2, 3, 4, 5, 6, 7]), "ff");
    }

    #[test]
    fn format_cpumask_high_bit() {
        assert_eq!(format_cpumask(&[31]), "80000000");
    }

    #[test]
    fn format_cpumask_second_word() {
        assert_eq!(format_cpumask(&[32]), "1,00000000");
    }

    #[test]
    fn format_cpumask_spanning_words() {
        assert_eq!(format_cpumask(&[0, 32, 33]), "3,00000001");
    }

    #[test]
    fn format_cpumask_third_word() {
        assert_eq!(format_cpumask(&[64, 1]), "1,00000000,00000002");
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
    fn parse_cpu_id_list_simple() {
        assert_eq!(parse_cpu_id_list("0-3"), Some(vec![0, 1, 2, 3]));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_cpu_id_list_complex() {
        assert_eq!(
            parse_cpu_id_list("0-3,8-11\n"),
            Some(vec![0, 1, 2, 3, 8, 9, 10, 11])
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_cpu_id_list_individual() {
        assert_eq!(parse_cpu_id_list("0,2,4"), Some(vec![0, 2, 4]));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_cpu_id_list_invalid() {
        assert_eq!(parse_cpu_id_list("abc"), None);
        assert_eq!(parse_cpu_id_list("3-1"), None);
    }
}
