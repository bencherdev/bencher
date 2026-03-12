use std::time::Duration;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Debug, Clone)]
pub struct PlatformMetrics {
    pub cpu_steal_percent: Option<f64>,
    pub context_switch_rate: Option<f64>,
    pub is_virtualized: Option<bool>,
    pub virtualization_type: Option<VirtualizationType>,
    pub cache_sizes: CacheSizes,
}

#[derive(Debug, Clone, Copy)]
#[expect(
    dead_code,
    reason = "Variants are constructed in platform-specific cfg modules (linux, macos, windows)"
)]
pub enum VirtualizationType {
    Docker,
    Container,
    Hypervisor,
    Kvm,
    Vmm,
    Vmware,
    Other,
}

impl VirtualizationType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Docker => "Docker",
            Self::Container => "Container",
            Self::Hypervisor => "Hypervisor",
            Self::Kvm => "KVM",
            Self::Vmm => "VMM",
            Self::Vmware => "VMware",
            Self::Other => "Other",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CacheSizes {
    pub l1d: Option<usize>,
    pub l2: Option<usize>,
    pub l3: Option<usize>,
}

pub fn detect_cache_sizes() -> CacheSizes {
    #[cfg(target_os = "linux")]
    {
        linux::detect_cache_sizes()
    }
    #[cfg(target_os = "macos")]
    {
        macos::detect_cache_sizes()
    }
    #[cfg(target_os = "windows")]
    {
        windows::detect_cache_sizes()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        CacheSizes::default()
    }
}

pub fn collect_metrics(duration: Duration) -> PlatformMetrics {
    #[cfg(target_os = "linux")]
    {
        linux::collect_metrics(duration)
    }
    #[cfg(target_os = "macos")]
    {
        macos::collect_metrics(duration)
    }
    #[cfg(target_os = "windows")]
    {
        windows::collect_metrics(duration)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        std::thread::sleep(duration);
        PlatformMetrics {
            cpu_steal_percent: None,
            context_switch_rate: None,
            is_virtualized: None,
            virtualization_type: None,
            cache_sizes: CacheSizes::default(),
        }
    }
}
