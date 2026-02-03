//! Run metrics collection.
//!
//! Collects timing and resource usage metrics during benchmark execution.
//! Metrics are output as structured JSON on stderr for diagnostic purposes.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Metrics collected during a benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMetrics {
    /// Total wall clock time for the VMM execution in milliseconds.
    pub wall_clock_ms: u64,

    /// Whether the execution timed out.
    pub timed_out: bool,

    /// Transport used to collect results ("vsock" or "serial").
    pub transport: String,

    /// Cgroup resource usage (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cgroup: Option<CgroupMetrics>,
}

/// Resource metrics from cgroup v2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CgroupMetrics {
    /// Total CPU usage in microseconds (user + system).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_usage_us: Option<u64>,

    /// User CPU time in microseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_user_us: Option<u64>,

    /// System CPU time in microseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_system_us: Option<u64>,

    /// Peak memory usage in bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_peak_bytes: Option<u64>,
}

/// Read cgroup metrics from the given cgroup path.
///
/// Reads `cpu.stat` and `memory.peak` from the cgroup directory.
/// Returns `None` if the path doesn't exist.
pub fn read_cgroup_metrics(cgroup_path: &Path) -> Option<CgroupMetrics> {
    if !cgroup_path.exists() {
        return None;
    }

    let cpu_stat = read_cpu_stat(cgroup_path);
    let memory_peak = read_file_u64(&cgroup_path.join("memory.peak"));

    Some(CgroupMetrics {
        cpu_usage_us: cpu_stat.as_ref().map(|s| s.usage_usec),
        cpu_user_us: cpu_stat.as_ref().map(|s| s.user_usec),
        cpu_system_us: cpu_stat.as_ref().map(|s| s.system_usec),
        memory_peak_bytes: memory_peak,
    })
}

/// Serialize metrics to the stderr marker format.
///
/// Format: `---BENCHER_METRICS:{json}---`
pub fn format_metrics(metrics: &RunMetrics) -> Option<String> {
    let json = serde_json::to_string(metrics).ok()?;
    Some(format!("---BENCHER_METRICS:{json}---"))
}

#[expect(clippy::struct_field_names, reason = "matches cgroup cpu.stat field names")]
struct CpuStat {
    usage_usec: u64,
    user_usec: u64,
    system_usec: u64,
}

fn read_cpu_stat(cgroup_path: &Path) -> Option<CpuStat> {
    let content = std::fs::read_to_string(cgroup_path.join("cpu.stat")).ok()?;
    let mut usage = None;
    let mut user = None;
    let mut system = None;

    for line in content.lines() {
        let mut parts = line.split_whitespace();
        match (parts.next(), parts.next()) {
            (Some("usage_usec"), Some(v)) => usage = v.parse().ok(),
            (Some("user_usec"), Some(v)) => user = v.parse().ok(),
            (Some("system_usec"), Some(v)) => system = v.parse().ok(),
            _ => {}
        }
    }

    Some(CpuStat {
        usage_usec: usage.unwrap_or(0),
        user_usec: user.unwrap_or(0),
        system_usec: system.unwrap_or(0),
    })
}

fn read_file_u64(path: &Path) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}
