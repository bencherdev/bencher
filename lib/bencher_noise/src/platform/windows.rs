use std::time::Duration;

use super::{CacheSizes, PlatformMetrics, VirtualizationType};

pub fn detect_cache_sizes() -> CacheSizes {
    // On Windows, we could use GetLogicalProcessorInformation but that requires
    // the windows crate with specific features. For now, return defaults.
    CacheSizes {
        l1d: None,
        l2: None,
        l3: None,
    }
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
    // Check common VM indicators via environment or system info
    // On Windows, WMI queries would be ideal but require COM initialization.
    // For now, check the PROCESSOR_IDENTIFIER environment variable for common VM strings.
    if let Ok(model) = std::env::var("COMPUTERNAME") {
        let model_lower = model.to_lowercase();
        if model_lower.contains("virtual") || model_lower.contains("vmware") {
            return (Some(true), Some(VirtualizationType::Other));
        }
    }

    (None, None)
}
