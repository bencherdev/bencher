use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

/// Configuration for a benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Path to the OCI image directory or archive.
    pub oci_image: Utf8PathBuf,

    /// Path to the Linux kernel to boot.
    pub kernel: Utf8PathBuf,

    /// Number of vCPUs to allocate to the VM.
    #[serde(default = "default_vcpus")]
    pub vcpus: u8,

    /// Memory size in MiB.
    #[serde(default = "default_memory_mib")]
    pub memory_mib: u32,

    /// Kernel command line arguments.
    #[serde(default = "default_kernel_cmdline")]
    pub kernel_cmdline: String,
}

const fn default_vcpus() -> u8 {
    1
}

const fn default_memory_mib() -> u32 {
    512
}

fn default_kernel_cmdline() -> String {
    "console=ttyS0 reboot=k panic=1 pci=off".to_owned()
}

impl Config {
    /// Create a new configuration with required fields.
    #[must_use]
    pub fn new(oci_image: Utf8PathBuf, kernel: Utf8PathBuf) -> Self {
        Self {
            oci_image,
            kernel,
            vcpus: default_vcpus(),
            memory_mib: default_memory_mib(),
            kernel_cmdline: default_kernel_cmdline(),
        }
    }

    /// Set the number of vCPUs.
    #[must_use]
    pub fn with_vcpus(mut self, vcpus: u8) -> Self {
        self.vcpus = vcpus;
        self
    }

    /// Set the memory size in MiB.
    #[must_use]
    pub fn with_memory_mib(mut self, memory_mib: u32) -> Self {
        self.memory_mib = memory_mib;
        self
    }

    /// Set the kernel command line.
    #[must_use]
    pub fn with_kernel_cmdline<S: Into<String>>(mut self, cmdline: S) -> Self {
        self.kernel_cmdline = cmdline.into();
        self
    }
}
