//! Run metrics collection.
//!
//! Collects timing and resource usage metrics during benchmark execution.
//! Metrics are output as structured JSON on stderr for diagnostic purposes.

use camino::Utf8Path;

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
/// Returns `None` when there is no cgroup to read.
///
/// Every field is optional, and a field that could not be read stays absent
/// rather than becoming a number. These are reported to an operator as measured
/// values, so a zero standing in for a failed read is the plainest form of the
/// one thing measurement is never allowed to do. Absence is already how this
/// reports "no cgroup at all", so it costs nothing to be honest per field.
///
/// That is also why the stat below is the one gating read in this crate that may
/// stay: it can only withhold a reading, never invent one. Nothing here can
/// answer with a number it did not read, so a failure reaches the operator as a
/// field that is not there. A stat that fails does not suppress the reads either,
/// which is the one thing worth tightening: only a stat that succeeded and said
/// absent means there is nothing to read, and anything else goes on to ask the
/// files themselves, since they report what they can and nothing more.
pub fn read_cgroup_metrics(cgroup_path: &Utf8Path) -> Option<CgroupMetrics> {
    if cgroup_path.try_exists().is_ok_and(|exists| !exists) {
        return None;
    }

    let cpu_stat = read_cpu_stat(cgroup_path).unwrap_or_default();
    let memory_peak = read_file_u64(&cgroup_path.join("memory.peak"));

    Some(CgroupMetrics {
        cpu_usage_us: cpu_stat.usage_usec,
        cpu_user_us: cpu_stat.user_usec,
        cpu_system_us: cpu_stat.system_usec,
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

/// The three fields of `cpu.stat` this runner reports.
///
/// Each one is what the file said, or nothing. A field the file did not carry, or
/// carried unparseably, is not zero usage: zero is a measurement, and this never
/// measured it.
#[derive(Default)]
#[expect(
    clippy::struct_field_names,
    reason = "matches cgroup cpu.stat field names"
)]
struct CpuStat {
    usage_usec: Option<u64>,
    user_usec: Option<u64>,
    system_usec: Option<u64>,
}

fn read_cpu_stat(cgroup_path: &Utf8Path) -> Option<CpuStat> {
    let content = std::fs::read_to_string(cgroup_path.join("cpu.stat")).ok()?;
    let mut stat = CpuStat::default();

    for line in content.lines() {
        let mut parts = line.split_whitespace();
        match (parts.next(), parts.next()) {
            (Some("usage_usec"), Some(v)) => stat.usage_usec = v.parse().ok(),
            (Some("user_usec"), Some(v)) => stat.user_usec = v.parse().ok(),
            (Some("system_usec"), Some(v)) => stat.system_usec = v.parse().ok(),
            _ => {},
        }
    }

    Some(stat)
}

fn read_file_u64(path: &Utf8Path) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    fn tempdir_utf8(dir: &tempfile::TempDir) -> &Utf8Path {
        Utf8Path::from_path(dir.path()).expect("tempdir is UTF-8")
    }

    // --- read_cpu_stat ---

    #[test]
    fn read_cpu_stat_normal() {
        let dir = tempfile::tempdir().unwrap();
        let path = tempdir_utf8(&dir);
        let content = "usage_usec 12345\nuser_usec 6000\nsystem_usec 6345\nnr_periods 0\n";
        fs::write(path.join("cpu.stat"), content).unwrap();

        let stat = read_cpu_stat(path).unwrap();
        assert_eq!(stat.usage_usec, Some(12345));
        assert_eq!(stat.user_usec, Some(6000));
        assert_eq!(stat.system_usec, Some(6345));
    }

    #[test]
    fn a_field_the_file_did_not_carry_is_absent_not_zero() {
        // Zero is a measurement. A field that was never read has to reach the
        // operator as missing, which is what the reported type already allows.
        let dir = tempfile::tempdir().unwrap();
        let path = tempdir_utf8(&dir);
        fs::write(path.join("cpu.stat"), "usage_usec 100\n").unwrap();

        let stat = read_cpu_stat(path).unwrap();
        assert_eq!(stat.usage_usec, Some(100));
        assert_eq!(stat.user_usec, None);
        assert_eq!(stat.system_usec, None);
    }

    #[test]
    fn read_cpu_stat_malformed_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = tempdir_utf8(&dir);
        fs::write(
            path.join("cpu.stat"),
            "usage_usec not_a_number\nuser_usec 100\nsystem_usec\n",
        )
        .unwrap();

        let stat = read_cpu_stat(path).unwrap();
        assert_eq!(stat.usage_usec, None, "a value that would not parse");
        assert_eq!(stat.user_usec, Some(100));
        assert_eq!(stat.system_usec, None, "no value at all");
    }

    #[test]
    fn read_cpu_stat_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = tempdir_utf8(&dir);
        fs::write(path.join("cpu.stat"), "").unwrap();

        let stat = read_cpu_stat(path).unwrap();
        assert_eq!(stat.usage_usec, None);
        assert_eq!(stat.user_usec, None);
        assert_eq!(stat.system_usec, None);
    }

    #[test]
    fn read_cpu_stat_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = tempdir_utf8(&dir);
        assert!(read_cpu_stat(path).is_none());
    }

    // --- read_file_u64 ---

    #[test]
    fn read_file_u64_normal() {
        let dir = tempfile::tempdir().unwrap();
        let path = tempdir_utf8(&dir).join("value");
        fs::write(&path, "42\n").unwrap();
        assert_eq!(read_file_u64(&path), Some(42));
    }

    #[test]
    fn read_file_u64_with_whitespace() {
        let dir = tempfile::tempdir().unwrap();
        let path = tempdir_utf8(&dir).join("value");
        fs::write(&path, "  1024  \n").unwrap();
        assert_eq!(read_file_u64(&path), Some(1024));
    }

    #[test]
    fn read_file_u64_non_numeric() {
        let dir = tempfile::tempdir().unwrap();
        let path = tempdir_utf8(&dir).join("value");
        fs::write(&path, "not_a_number").unwrap();
        assert_eq!(read_file_u64(&path), None);
    }

    #[test]
    fn read_file_u64_missing_file() {
        assert_eq!(read_file_u64(Utf8Path::new("/nonexistent/path")), None);
    }

    #[test]
    fn read_file_u64_negative_number() {
        let dir = tempfile::tempdir().unwrap();
        let path = tempdir_utf8(&dir).join("value");
        fs::write(&path, "-1\n").unwrap();
        assert_eq!(read_file_u64(&path), None); // u64 can't parse negative
    }

    // --- read_cgroup_metrics ---

    #[test]
    fn read_cgroup_metrics_full() {
        let dir = tempfile::tempdir().unwrap();
        let path = tempdir_utf8(&dir);
        fs::write(
            path.join("cpu.stat"),
            "usage_usec 5000\nuser_usec 3000\nsystem_usec 2000\n",
        )
        .unwrap();
        fs::write(path.join("memory.peak"), "1048576\n").unwrap();

        let metrics = read_cgroup_metrics(path).unwrap();
        assert_eq!(metrics.cpu_usage_us, Some(5000));
        assert_eq!(metrics.cpu_user_us, Some(3000));
        assert_eq!(metrics.cpu_system_us, Some(2000));
        assert_eq!(metrics.memory_peak_bytes, Some(0x0010_0000));
    }

    #[test]
    fn read_cgroup_metrics_no_memory_peak() {
        let dir = tempfile::tempdir().unwrap();
        let path = tempdir_utf8(&dir);
        fs::write(
            path.join("cpu.stat"),
            "usage_usec 100\nuser_usec 50\nsystem_usec 50\n",
        )
        .unwrap();

        let metrics = read_cgroup_metrics(path).unwrap();
        assert_eq!(metrics.cpu_usage_us, Some(100));
        assert_eq!(metrics.memory_peak_bytes, None);
    }

    #[test]
    fn read_cgroup_metrics_nonexistent_path() {
        assert!(read_cgroup_metrics(Utf8Path::new("/nonexistent")).is_none());
    }

    #[test]
    fn a_cgroup_whose_files_cannot_be_read_reports_no_numbers() {
        // The property that makes this module's gating stat harmless: every
        // failure here withholds a field, and none of them invents one. An empty
        // reading is honest; a zero would not be.
        let dir = tempfile::tempdir().unwrap();
        let path = tempdir_utf8(&dir);

        let metrics = read_cgroup_metrics(path).unwrap();

        assert_eq!(metrics.cpu_usage_us, None);
        assert_eq!(metrics.cpu_user_us, None);
        assert_eq!(metrics.cpu_system_us, None);
        assert_eq!(metrics.memory_peak_bytes, None);
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
