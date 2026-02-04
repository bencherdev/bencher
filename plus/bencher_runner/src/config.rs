use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

/// Configuration for a benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// OCI image source - either a local path or registry reference.
    ///
    /// Examples:
    /// - `/path/to/oci-image` (local directory)
    /// - `ghcr.io/owner/benchmark:v1` (registry reference)
    /// - `docker.io/library/alpine:latest` (Docker Hub)
    pub oci_image: String,

    /// Path to the Linux kernel to boot.
    ///
    /// If not specified, the bundled kernel will be used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel: Option<Utf8PathBuf>,

    /// JWT token for registry authentication.
    ///
    /// Required when pulling from authenticated registries.
    /// This token is exchanged for a short-lived bearer token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    /// Cache directory for pulled OCI images.
    ///
    /// Defaults to `/var/cache/bencher/oci` if not specified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_dir: Option<Utf8PathBuf>,

    /// Number of vCPUs to allocate to the VM.
    #[serde(default = "default_vcpus")]
    pub vcpus: u8,

    /// Memory size in MiB.
    #[serde(default = "default_memory_mib")]
    pub memory_mib: u32,

    /// Kernel command line arguments.
    #[serde(default = "default_kernel_cmdline")]
    pub kernel_cmdline: String,

    /// Timeout for benchmark execution in seconds.
    ///
    /// If the benchmark doesn't complete within this time, it will be killed.
    /// Defaults to 300 seconds (5 minutes).
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    /// Path to an output file the benchmark will produce.
    ///
    /// If specified, the init script will set `BENCHER_OUTPUT_FILE` environment
    /// variable, and the file will be sent to the host via vsock port 5005.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_file: Option<String>,
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

const fn default_timeout_secs() -> u64 {
    300 // 5 minutes
}

impl Config {
    /// Create a new configuration with the bundled kernel.
    ///
    /// # Arguments
    ///
    /// * `oci_image` - Local path or registry reference (e.g., `ghcr.io/owner/bench:v1`)
    #[must_use]
    pub fn new<S: Into<String>>(oci_image: S) -> Self {
        Self {
            oci_image: oci_image.into(),
            kernel: None,
            token: None,
            cache_dir: None,
            vcpus: default_vcpus(),
            memory_mib: default_memory_mib(),
            kernel_cmdline: default_kernel_cmdline(),
            timeout_secs: default_timeout_secs(),
            output_file: None,
        }
    }

    /// Create a new configuration with a custom kernel.
    ///
    /// # Arguments
    ///
    /// * `oci_image` - Local path or registry reference (e.g., `ghcr.io/owner/bench:v1`)
    /// * `kernel` - Path to the Linux kernel
    #[must_use]
    pub fn with_kernel<S: Into<String>>(oci_image: S, kernel: Utf8PathBuf) -> Self {
        Self {
            oci_image: oci_image.into(),
            kernel: Some(kernel),
            token: None,
            cache_dir: None,
            vcpus: default_vcpus(),
            memory_mib: default_memory_mib(),
            kernel_cmdline: default_kernel_cmdline(),
            timeout_secs: default_timeout_secs(),
            output_file: None,
        }
    }

    /// Set the JWT token for registry authentication.
    #[must_use]
    pub fn with_token<S: Into<String>>(mut self, token: S) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Set the cache directory for pulled images.
    #[must_use]
    pub fn with_cache_dir(mut self, cache_dir: Utf8PathBuf) -> Self {
        self.cache_dir = Some(cache_dir);
        self
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

    /// Set the timeout in seconds.
    #[must_use]
    pub fn with_timeout_secs(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    /// Set the output file path (inside the guest VM).
    ///
    /// When set, `BENCHER_OUTPUT_FILE` environment variable will be available
    /// to the benchmark, and the file's contents will be sent to the host
    /// via vsock port 5005 after the benchmark completes.
    #[must_use]
    pub fn with_output_file<S: Into<String>>(mut self, output_file: S) -> Self {
        self.output_file = Some(output_file.into());
        self
    }

    /// Get the cache directory, using the default if not set.
    #[must_use]
    pub fn cache_dir(&self) -> Utf8PathBuf {
        self.cache_dir
            .clone()
            .unwrap_or_else(|| Utf8PathBuf::from("/var/cache/bencher/oci"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_new_defaults() {
        let config = Config::new("my-image:latest");
        assert_eq!(config.oci_image, "my-image:latest");
        assert_eq!(config.vcpus, 1);
        assert_eq!(config.memory_mib, 512);
        assert_eq!(config.timeout_secs, 300);
        assert!(config.kernel.is_none());
        assert!(config.token.is_none());
        assert!(config.cache_dir.is_none());
        assert!(config.output_file.is_none());
    }

    #[test]
    fn config_with_kernel() {
        let config = Config::with_kernel("img", Utf8PathBuf::from("/boot/vmlinux"));
        assert_eq!(config.kernel.unwrap().as_str(), "/boot/vmlinux");
    }

    #[test]
    fn config_builder_chain() {
        let config = Config::new("img")
            .with_token("jwt-token")
            .with_vcpus(4)
            .with_memory_mib(2048)
            .with_timeout_secs(600)
            .with_output_file("/tmp/results.json")
            .with_cache_dir(Utf8PathBuf::from("/cache"));

        assert_eq!(config.token.unwrap(), "jwt-token");
        assert_eq!(config.vcpus, 4);
        assert_eq!(config.memory_mib, 2048);
        assert_eq!(config.timeout_secs, 600);
        assert_eq!(config.output_file.unwrap(), "/tmp/results.json");
        assert_eq!(config.cache_dir.unwrap().as_str(), "/cache");
    }

    #[test]
    fn cache_dir_default() {
        let config = Config::new("img");
        assert_eq!(config.cache_dir().as_str(), "/var/cache/bencher/oci");
    }

    #[test]
    fn cache_dir_custom() {
        let config = Config::new("img").with_cache_dir(Utf8PathBuf::from("/my/cache"));
        assert_eq!(config.cache_dir().as_str(), "/my/cache");
    }

    #[test]
    fn config_serde_round_trip() {
        let config = Config::new("ghcr.io/test/bench:v1")
            .with_vcpus(2)
            .with_memory_mib(1024)
            .with_output_file("/output.json");

        let json = serde_json::to_string(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.oci_image, "ghcr.io/test/bench:v1");
        assert_eq!(parsed.vcpus, 2);
        assert_eq!(parsed.memory_mib, 1024);
        assert_eq!(parsed.output_file.unwrap(), "/output.json");
        // Optional None fields should not appear in JSON
        assert!(!json.contains("\"token\""));
        assert!(!json.contains("\"kernel\""));
    }

    #[test]
    fn config_deserialize_with_defaults() {
        let json = r#"{"oci_image": "test:latest"}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.oci_image, "test:latest");
        assert_eq!(config.vcpus, 1);
        assert_eq!(config.memory_mib, 512);
        assert_eq!(config.timeout_secs, 300);
    }
}
