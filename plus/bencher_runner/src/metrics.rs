//! Run metrics collection.
//!
//! Collects timing and resource usage metrics during benchmark execution,
//! logged as the flat fields of one `Run metrics` record.

use camino::Utf8Path;

/// Metrics collected during a benchmark run.
#[derive(Debug, Clone)]
pub struct RunMetrics {
    /// Total wall clock time for the VMM execution in milliseconds.
    pub wall_clock_ms: u64,

    /// Whether the execution timed out.
    pub timed_out: bool,

    /// Transport used to collect results ("vsock" or "serial").
    pub transport: String,

    /// Cgroup resource usage (if available).
    pub cgroup: Option<CgroupMetrics>,
}

/// A field that was not measured is left out rather than logged empty.
impl slog::KV for RunMetrics {
    fn serialize(
        &self,
        record: &slog::Record<'_>,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        use slog::SingleKV;

        // `SingleKV` builds the key, whose type depends on slog's features.
        SingleKV::from(("wall_clock_ms", self.wall_clock_ms)).serialize(record, serializer)?;
        SingleKV::from(("timed_out", self.timed_out)).serialize(record, serializer)?;
        SingleKV::from(("transport", self.transport.as_str())).serialize(record, serializer)?;
        let Some(cgroup) = &self.cgroup else {
            return Ok(());
        };
        for (key, value) in [
            ("cpu_usage_us", cgroup.cpu_usage_us),
            ("cpu_user_us", cgroup.cpu_user_us),
            ("cpu_system_us", cgroup.cpu_system_us),
            ("memory_peak_bytes", cgroup.memory_peak_bytes),
        ] {
            if let Some(value) = value {
                SingleKV::from((key, value)).serialize(record, serializer)?;
            }
        }
        Ok(())
    }
}

/// Resource metrics from cgroup v2.
#[derive(Debug, Clone)]
pub struct CgroupMetrics {
    /// Total CPU usage in microseconds (user + system).
    pub cpu_usage_us: Option<u64>,

    /// User CPU time in microseconds.
    pub cpu_user_us: Option<u64>,

    /// System CPU time in microseconds.
    pub cpu_system_us: Option<u64>,

    /// Peak memory usage in bytes.
    pub memory_peak_bytes: Option<u64>,
}

/// Read cgroup metrics from the given cgroup path.
///
/// Reads `cpu.stat` and `memory.peak` from the cgroup directory.
/// Returns `None` only when the cgroup is confirmed absent, and a field that
/// could not be read stays `None` rather than reading as a measured zero.
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
        // A failed read that became zero would report a measurement never taken.
        let dir = tempfile::tempdir().unwrap();
        let path = tempdir_utf8(&dir);

        let metrics = read_cgroup_metrics(path).unwrap();

        assert_eq!(metrics.cpu_usage_us, None);
        assert_eq!(metrics.cpu_user_us, None);
        assert_eq!(metrics.cpu_system_us, None);
        assert_eq!(metrics.memory_peak_bytes, None);
    }
}
