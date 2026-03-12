use std::time::Duration;

use super::{CacheSizes, PlatformMetrics, VirtualizationType};

pub fn detect_cache_sizes() -> CacheSizes {
    CacheSizes {
        l1d: read_cache_size(0, "Data"),
        l2: read_cache_size(2, "Unified"),
        l3: read_cache_size(3, "Unified"),
    }
}

fn read_cache_size(index: u32, expected_type: &str) -> Option<usize> {
    let base = format!("/sys/devices/system/cpu/cpu0/cache/index{index}");
    let cache_type = std::fs::read_to_string(format!("{base}/type")).ok()?;
    if !cache_type.trim().eq_ignore_ascii_case(expected_type) {
        return None;
    }
    let size_str = std::fs::read_to_string(format!("{base}/size")).ok()?;
    parse_cache_size_str(size_str.trim())
}

fn parse_cache_size_str(s: &str) -> Option<usize> {
    if let Some(k) = s.strip_suffix('K') {
        k.parse::<usize>().ok().map(|v| v * 1024)
    } else if let Some(m) = s.strip_suffix('M') {
        m.parse::<usize>().ok().map(|v| v * 1024 * 1024)
    } else {
        s.parse::<usize>().ok()
    }
}

pub fn collect_metrics(duration: Duration) -> PlatformMetrics {
    let steal_start = read_steal_time();
    let ctxt_start = read_context_switches();

    std::thread::sleep(duration);

    let steal_end = read_steal_time();
    let ctxt_end = read_context_switches();

    let cpu_steal_percent = match (steal_start, steal_end) {
        (Some((steal_s, total_s)), Some((steal_e, total_e))) => {
            #[expect(
                clippy::cast_precision_loss,
                reason = "CPU tick counts fit well within f64 precision for percentage calculation"
            )]
            let delta_steal = steal_e.saturating_sub(steal_s) as f64;
            #[expect(
                clippy::cast_precision_loss,
                reason = "CPU tick counts fit well within f64 precision for percentage calculation"
            )]
            let delta_total = total_e.saturating_sub(total_s) as f64;
            (delta_total > 0.0).then(|| (delta_steal / delta_total) * 100.0)
        },
        _ => None,
    };

    let context_switch_rate = match (ctxt_start, ctxt_end) {
        (Some(start), Some(end)) => {
            #[expect(
                clippy::cast_precision_loss,
                reason = "context switch delta fits well within f64 precision for rate calculation"
            )]
            let delta = end.saturating_sub(start) as f64;
            let secs = duration.as_secs_f64();
            (secs > 0.0).then(|| delta / secs)
        },
        _ => None,
    };

    let (is_virtualized, virtualization_type) = detect_virtualization();

    PlatformMetrics {
        cpu_steal_percent,
        context_switch_rate,
        is_virtualized,
        virtualization_type,
        cache_sizes: detect_cache_sizes(),
    }
}

/// Parse `/proc/stat` to extract steal time.
/// Returns (`steal_ticks`, `total_ticks`) for the aggregate CPU line.
fn read_steal_time() -> Option<(u64, u64)> {
    let content = std::fs::read_to_string("/proc/stat").ok()?;
    parse_proc_stat(&content)
}

pub fn parse_proc_stat(content: &str) -> Option<(u64, u64)> {
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("cpu ") {
            let fields: Vec<u64> = rest
                .split_whitespace()
                .filter_map(|f| f.parse().ok())
                .collect();
            // Fields: user, nice, system, idle, iowait, irq, softirq, steal, ...
            let steal = fields.get(7).copied()?;
            let total: u64 = fields.iter().sum();
            return Some((steal, total));
        }
    }
    None
}

fn read_context_switches() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/vmstat").ok()?;
    parse_proc_vmstat_ctxt(&content)
}

pub fn parse_proc_vmstat_ctxt(content: &str) -> Option<u64> {
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("ctxt ") {
            return rest.trim().parse().ok();
        }
    }
    None
}

fn detect_virtualization() -> (Option<bool>, Option<VirtualizationType>) {
    // Check for Docker/container
    if std::fs::metadata("/.dockerenv").is_ok() {
        return (Some(true), Some(VirtualizationType::Docker));
    }
    if std::fs::metadata("/run/.containerenv").is_ok() {
        return (Some(true), Some(VirtualizationType::Container));
    }

    // Check DMI product name
    if let Ok(product) = std::fs::read_to_string("/sys/class/dmi/id/product_name") {
        let product = product.trim().to_lowercase();
        if product.contains("kvm") {
            return (Some(true), Some(VirtualizationType::Kvm));
        }
        if product.contains("vmware") {
            return (Some(true), Some(VirtualizationType::Vmware));
        }
        if product.contains("virtual") {
            return (Some(true), Some(VirtualizationType::Other));
        }
    }

    // Check /proc/cpuinfo for hypervisor flag
    if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
        for line in cpuinfo.lines() {
            if line.starts_with("flags") && line.contains("hypervisor") {
                return (Some(true), Some(VirtualizationType::Hypervisor));
            }
        }
    }

    (Some(false), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proc_stat() {
        let content = "cpu  10132153 290696 3084719 46828483 16683 0 25195 5765 0 0\n\
                        cpu0 1393280 32966 572056 13343292 6130 0 17875 2093 0 0\n";
        let (steal, total) = parse_proc_stat(content).unwrap();
        assert_eq!(steal, 5_765);
        assert_eq!(
            total,
            10_132_153 + 290_696 + 3_084_719 + 46_828_483 + 16_683 + 25_195 + 5_765
        );
    }

    #[test]
    fn parse_proc_stat_insufficient_fields() {
        let content = "cpu  100 200 300\n";
        assert!(parse_proc_stat(content).is_none());
    }

    #[test]
    fn parses_proc_vmstat_ctxt() {
        let content = "nr_free_pages 12345\nctxt 987654321\nnr_inactive_anon 100\n";
        assert_eq!(parse_proc_vmstat_ctxt(content), Some(987_654_321));
    }

    #[test]
    fn parse_proc_vmstat_ctxt_missing() {
        let content = "nr_free_pages 12345\n";
        assert!(parse_proc_vmstat_ctxt(content).is_none());
    }

    #[test]
    fn parses_cache_size_str() {
        assert_eq!(parse_cache_size_str("32K"), Some(32 * 1024));
        assert_eq!(parse_cache_size_str("8M"), Some(8 * 1024 * 1024));
        assert_eq!(parse_cache_size_str("65536"), Some(0x1_0000));
        assert_eq!(parse_cache_size_str("invalid"), None);
    }
}
