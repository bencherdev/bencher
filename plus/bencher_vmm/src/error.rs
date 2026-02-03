use thiserror::Error;

#[derive(Debug, Error)]
pub enum VmmError {
    #[cfg(target_os = "linux")]
    #[error("KVM error: {0}")]
    Kvm(#[from] kvm_ioctls::Error),

    #[error("Memory error: {0}")]
    Memory(String),

    #[error("Boot error: {0}")]
    Boot(String),

    #[error("Device error: {0}")]
    Device(String),

    #[error("vCPU error: {0}")]
    Vcpu(String),

    #[error("GIC error: {0}")]
    Gic(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Kernel loading error: {0}")]
    KernelLoad(String),

    #[error("Unsupported architecture")]
    UnsupportedArch,

    #[error("VMM requires Linux with KVM support")]
    UnsupportedPlatform,

    #[error("VM execution timed out after {timeout_secs} seconds")]
    Timeout {
        timeout_secs: u64,
        /// Any partial output captured before the timeout.
        partial_output: String,
    },

    #[error("Sandbox error: {0}")]
    Sandbox(String),
}
