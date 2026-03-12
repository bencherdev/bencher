use std::time::Duration;

use super::{CacheSizes, PlatformMetrics, VirtualizationType};

pub fn detect_cache_sizes() -> CacheSizes {
    CacheSizes {
        l1d: sysctl_usize("hw.l1dcachesize"),
        l2: sysctl_usize("hw.l2cachesize"),
        l3: sysctl_usize("hw.l3cachesize"),
    }
}

fn sysctl_usize(name: &str) -> Option<usize> {
    let output = std::process::Command::new("sysctl")
        .arg("-n")
        .arg(name)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?;
    s.trim().parse().ok()
}

pub fn collect_metrics(duration: Duration) -> PlatformMetrics {
    std::thread::sleep(duration);

    let (is_virtualized, virtualization_type) = detect_virtualization();

    PlatformMetrics {
        cpu_steal_percent: None,
        context_switch_rate: None,
        is_virtualized,
        virtualization_type,
        cache_sizes: detect_cache_sizes(),
    }
}

fn detect_virtualization() -> (Option<bool>, Option<VirtualizationType>) {
    // Check kern.hv_vmm_present (Hypervisor.framework)
    if sysctl_usize("kern.hv_vmm_present") == Some(1) {
        return (Some(true), Some(VirtualizationType::Hypervisor));
    }

    // Check for VM in sysctl machdep.cpu.features
    if let Ok(output) = std::process::Command::new("sysctl")
        .arg("-n")
        .arg("machdep.cpu.features")
        .output()
        && output.status.success()
        && String::from_utf8_lossy(&output.stdout).contains("VMM")
    {
        return (Some(true), Some(VirtualizationType::Vmm));
    }

    (Some(false), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_cache_sizes_runs() {
        // Just verify it doesn't panic
        let sizes = detect_cache_sizes();
        let _ = sizes;
    }
}
