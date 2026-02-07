use std::collections::HashMap;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::cpu::CpuLayout;

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

    /// Number of vCPUs to allocate to the VM.
    #[serde(default = "default_vcpus")]
    pub vcpus: u8,

    /// Memory size in MiB.
    #[serde(default = "default_memory_mib")]
    pub memory_mib: u32,

    /// Disk size in MiB.
    #[serde(default = "default_disk_mib")]
    pub disk_mib: u32,

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

    /// Whether to enable network access in the VM.
    #[serde(default)]
    pub network: bool,

    /// Optional entrypoint override for the container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Vec<String>>,

    /// Optional command override for the container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmd: Option<Vec<String>>,

    /// Optional environment variables for the container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    /// Maximum size in bytes for collected stdout/stderr inside the guest VM.
    ///
    /// If a benchmark produces more output than this limit, the excess is
    /// silently dropped. Defaults to 10 MiB, matching the host-side vsock
    /// `MAX_DATA_SIZE`.
    #[serde(default = "default_max_output_size")]
    pub max_output_size: usize,

    /// CPU layout for isolating benchmark cores from housekeeping tasks.
    ///
    /// When set, the Firecracker process will be pinned to benchmark cores
    /// via cgroups cpuset. This field is not serialized.
    #[serde(skip)]
    pub cpu_layout: Option<CpuLayout>,
}

const fn default_vcpus() -> u8 {
    1
}

const fn default_memory_mib() -> u32 {
    512
}

const fn default_disk_mib() -> u32 {
    1024 // 1 GiB
}

fn default_kernel_cmdline() -> String {
    "console=ttyS0 reboot=t panic=1 pci=off root=/dev/vda rw init=/init".to_owned()
}

const fn default_timeout_secs() -> u64 {
    300 // 5 minutes
}

const fn default_max_output_size() -> usize {
    10 * 1024 * 1024 // 10 MiB
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
            vcpus: default_vcpus(),
            memory_mib: default_memory_mib(),
            disk_mib: default_disk_mib(),
            kernel_cmdline: default_kernel_cmdline(),
            timeout_secs: default_timeout_secs(),
            output_file: None,
            network: false,
            entrypoint: None,
            cmd: None,
            env: None,
            max_output_size: default_max_output_size(),
            cpu_layout: None,
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
            vcpus: default_vcpus(),
            memory_mib: default_memory_mib(),
            disk_mib: default_disk_mib(),
            kernel_cmdline: default_kernel_cmdline(),
            timeout_secs: default_timeout_secs(),
            output_file: None,
            network: false,
            entrypoint: None,
            cmd: None,
            env: None,
            max_output_size: default_max_output_size(),
            cpu_layout: None,
        }
    }

    /// Set the JWT token for registry authentication.
    #[must_use]
    pub fn with_token<S: Into<String>>(mut self, token: S) -> Self {
        self.token = Some(token.into());
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

    /// Set the disk size in MiB.
    #[must_use]
    pub fn with_disk_mib(mut self, disk_mib: u32) -> Self {
        self.disk_mib = disk_mib;
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

    /// Enable or disable network access in the VM.
    #[must_use]
    pub fn with_network(mut self, network: bool) -> Self {
        self.network = network;
        self
    }

    /// Set the entrypoint override for the container.
    #[must_use]
    pub fn with_entrypoint(mut self, entrypoint: Vec<String>) -> Self {
        self.entrypoint = Some(entrypoint);
        self
    }

    /// Set the entrypoint override for the container from an Option.
    #[must_use]
    pub fn with_entrypoint_opt(mut self, entrypoint: Option<Vec<String>>) -> Self {
        self.entrypoint = entrypoint;
        self
    }

    /// Set the command override for the container.
    #[must_use]
    pub fn with_cmd(mut self, cmd: Vec<String>) -> Self {
        self.cmd = Some(cmd);
        self
    }

    /// Set the command override for the container from an Option.
    #[must_use]
    pub fn with_cmd_opt(mut self, cmd: Option<Vec<String>>) -> Self {
        self.cmd = cmd;
        self
    }

    /// Set the environment variables for the container.
    #[must_use]
    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = Some(env);
        self
    }

    /// Set the environment variables for the container from an Option.
    #[must_use]
    pub fn with_env_opt(mut self, env: Option<HashMap<String, String>>) -> Self {
        self.env = env;
        self
    }

    /// Set the maximum output size in bytes for stdout/stderr collection.
    #[must_use]
    pub fn with_max_output_size(mut self, max_output_size: usize) -> Self {
        self.max_output_size = max_output_size;
        self
    }

    /// Set the CPU layout for core isolation.
    ///
    /// When set, the Firecracker process will be pinned to benchmark cores
    /// via cgroups cpuset, isolating it from housekeeping tasks.
    #[must_use]
    pub fn with_cpu_layout(mut self, layout: CpuLayout) -> Self {
        self.cpu_layout = Some(layout);
        self
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
        assert_eq!(config.disk_mib, 1024);
        assert_eq!(config.timeout_secs, 300);
        assert!(!config.network);
        assert!(config.kernel.is_none());
        assert!(config.token.is_none());
        assert!(config.output_file.is_none());
        assert!(config.entrypoint.is_none());
        assert!(config.cmd.is_none());
        assert!(config.env.is_none());
        assert!(config.cpu_layout.is_none());
    }

    #[test]
    fn config_with_kernel() {
        let config = Config::with_kernel("img", Utf8PathBuf::from("/boot/vmlinux"));
        assert_eq!(config.kernel.unwrap().as_str(), "/boot/vmlinux");
    }

    #[test]
    fn config_builder_chain() {
        let env: HashMap<String, String> = [("RUST_LOG".to_owned(), "debug".to_owned())].into();
        let config = Config::new("img")
            .with_token("jwt-token")
            .with_vcpus(4)
            .with_memory_mib(2048)
            .with_disk_mib(4096)
            .with_timeout_secs(600)
            .with_output_file("/tmp/results.json")
            .with_network(true)
            .with_entrypoint(vec!["/bin/sh".to_owned()])
            .with_cmd(vec!["-c".to_owned(), "echo hello".to_owned()])
            .with_env(env.clone());

        assert_eq!(config.token.unwrap(), "jwt-token");
        assert_eq!(config.vcpus, 4);
        assert_eq!(config.memory_mib, 2048);
        assert_eq!(config.disk_mib, 4096);
        assert_eq!(config.timeout_secs, 600);
        assert_eq!(config.output_file.unwrap(), "/tmp/results.json");
        assert!(config.network);
        assert_eq!(config.entrypoint.unwrap(), vec!["/bin/sh"]);
        assert_eq!(config.cmd.unwrap(), vec!["-c", "echo hello"]);
        assert_eq!(config.env.unwrap(), env);
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
        assert_eq!(config.disk_mib, 1024);
        assert_eq!(config.timeout_secs, 300);
        assert!(!config.network);
    }

    #[test]
    fn config_opt_builders() {
        // Test *_opt builders with Some values
        let config = Config::new("img")
            .with_entrypoint_opt(Some(vec!["/bin/bash".to_owned()]))
            .with_cmd_opt(Some(vec!["test".to_owned()]))
            .with_env_opt(Some([("KEY".to_owned(), "value".to_owned())].into()));

        assert_eq!(config.entrypoint.unwrap(), vec!["/bin/bash"]);
        assert_eq!(config.cmd.unwrap(), vec!["test"]);
        assert_eq!(config.env.unwrap().get("KEY").unwrap(), "value");

        // Test *_opt builders with None values
        let config = Config::new("img")
            .with_entrypoint_opt(None)
            .with_cmd_opt(None)
            .with_env_opt(None);

        assert!(config.entrypoint.is_none());
        assert!(config.cmd.is_none());
        assert!(config.env.is_none());
    }
}
