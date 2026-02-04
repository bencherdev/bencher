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

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    // --- read_cpu_stat ---

    #[test]
    fn read_cpu_stat_normal() {
        let dir = tempfile::tempdir().unwrap();
        let content = "usage_usec 12345\nuser_usec 6000\nsystem_usec 6345\nnr_periods 0\n";
        fs::write(dir.path().join("cpu.stat"), content).unwrap();

        let stat = read_cpu_stat(dir.path()).unwrap();
        assert_eq!(stat.usage_usec, 12345);
        assert_eq!(stat.user_usec, 6000);
        assert_eq!(stat.system_usec, 6345);
    }

    #[test]
    fn read_cpu_stat_missing_fields_default_to_zero() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("cpu.stat"), "usage_usec 100\n").unwrap();

        let stat = read_cpu_stat(dir.path()).unwrap();
        assert_eq!(stat.usage_usec, 100);
        assert_eq!(stat.user_usec, 0);
        assert_eq!(stat.system_usec, 0);
    }

    #[test]
    fn read_cpu_stat_malformed_values() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("cpu.stat"),
            "usage_usec not_a_number\nuser_usec 100\nsystem_usec\n",
        )
        .unwrap();

        let stat = read_cpu_stat(dir.path()).unwrap();
        assert_eq!(stat.usage_usec, 0); // parse fails -> unwrap_or(0)
        assert_eq!(stat.user_usec, 100);
        assert_eq!(stat.system_usec, 0); // no value at all
    }

    #[test]
    fn read_cpu_stat_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("cpu.stat"), "").unwrap();

        let stat = read_cpu_stat(dir.path()).unwrap();
        assert_eq!(stat.usage_usec, 0);
        assert_eq!(stat.user_usec, 0);
        assert_eq!(stat.system_usec, 0);
    }

    #[test]
    fn read_cpu_stat_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_cpu_stat(dir.path()).is_none());
    }

    // --- read_file_u64 ---

    #[test]
    fn read_file_u64_normal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("value");
        fs::write(&path, "42\n").unwrap();
        assert_eq!(read_file_u64(&path), Some(42));
    }

    #[test]
    fn read_file_u64_with_whitespace() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("value");
        fs::write(&path, "  1024  \n").unwrap();
        assert_eq!(read_file_u64(&path), Some(1024));
    }

    #[test]
    fn read_file_u64_non_numeric() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("value");
        fs::write(&path, "not_a_number").unwrap();
        assert_eq!(read_file_u64(&path), None);
    }

    #[test]
    fn read_file_u64_missing_file() {
        assert_eq!(read_file_u64(Path::new("/nonexistent/path")), None);
    }

    #[test]
    fn read_file_u64_negative_number() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("value");
        fs::write(&path, "-1\n").unwrap();
        assert_eq!(read_file_u64(&path), None); // u64 can't parse negative
    }

    // --- read_cgroup_metrics ---

    #[test]
    fn read_cgroup_metrics_full() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("cpu.stat"),
            "usage_usec 5000\nuser_usec 3000\nsystem_usec 2000\n",
        )
        .unwrap();
        fs::write(dir.path().join("memory.peak"), "1048576\n").unwrap();

        let metrics = read_cgroup_metrics(dir.path()).unwrap();
        assert_eq!(metrics.cpu_usage_us, Some(5000));
        assert_eq!(metrics.cpu_user_us, Some(3000));
        assert_eq!(metrics.cpu_system_us, Some(2000));
        assert_eq!(metrics.memory_peak_bytes, Some(1_048_576));
    }

    #[test]
    fn read_cgroup_metrics_no_memory_peak() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("cpu.stat"),
            "usage_usec 100\nuser_usec 50\nsystem_usec 50\n",
        )
        .unwrap();

        let metrics = read_cgroup_metrics(dir.path()).unwrap();
        assert_eq!(metrics.cpu_usage_us, Some(100));
        assert_eq!(metrics.memory_peak_bytes, None);
    }

    #[test]
    fn read_cgroup_metrics_nonexistent_path() {
        assert!(read_cgroup_metrics(Path::new("/nonexistent")).is_none());
    }

    // --- format_metrics ---

    #[test]
    fn format_metrics_round_trip() {
        let metrics = RunMetrics {
            wall_clock_ms: 1500,
            timed_out: false,
            transport: "vsock".to_owned(),
            cgroup: Some(CgroupMetrics {
                cpu_usage_us: Some(1000),
                cpu_user_us: Some(600),
                cpu_system_us: Some(400),
                memory_peak_bytes: Some(2048),
            }),
        };
        let formatted = format_metrics(&metrics).unwrap();
        assert!(formatted.starts_with("---BENCHER_METRICS:"));
        assert!(formatted.ends_with("---"));

        // Extract JSON and verify it parses back
        let json = formatted
            .strip_prefix("---BENCHER_METRICS:")
            .unwrap()
            .strip_suffix("---")
            .unwrap();
        let parsed: RunMetrics = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.wall_clock_ms, 1500);
        assert!(!parsed.timed_out);
        assert_eq!(parsed.transport, "vsock");
        assert_eq!(parsed.cgroup.as_ref().unwrap().cpu_usage_us, Some(1000));
    }

    #[test]
    fn format_metrics_no_cgroup() {
        let metrics = RunMetrics {
            wall_clock_ms: 500,
            timed_out: true,
            transport: "vsock".to_owned(),
            cgroup: None,
        };
        let formatted = format_metrics(&metrics).unwrap();
        // cgroup should be absent from JSON (skip_serializing_if)
        assert!(!formatted.contains("\"cgroup\""));
        assert!(formatted.contains("\"timed_out\":true"));
    }
}
