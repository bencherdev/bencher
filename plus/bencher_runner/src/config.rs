use std::collections::HashMap;

use bencher_json::{Cpu, Disk, Memory};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::cpu::CpuLayout;
#[cfg(target_os = "linux")]
use crate::firecracker::FirecrackerLogLevel;

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
    pub vcpus: Cpu,

    /// Memory size in bytes.
    #[serde(default = "default_memory")]
    pub memory: Memory,

    /// Disk size in bytes.
    #[serde(default = "default_disk")]
    pub disk: Disk,

    /// Kernel command line arguments.
    #[serde(default = "default_kernel_cmdline")]
    pub kernel_cmdline: String,

    /// Timeout for benchmark execution in seconds.
    ///
    /// If the benchmark doesn't complete within this time, it will be killed.
    /// Defaults to 300 seconds (5 minutes).
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    /// Paths to output files the benchmark will produce.
    ///
    /// If specified, the files will be read by the init process and sent to
    /// the host via vsock port 5005 using a length-prefixed binary protocol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_paths: Option<Vec<Utf8PathBuf>>,

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

    /// Maximum size in bytes for collected stdout/stderr.
    ///
    /// This limit is enforced on both sides: the guest-side init process
    /// truncates output at this size, and the host-side vsock reader stops
    /// reading at the same cap. Defaults to 25 MiB.
    #[serde(default = "default_max_output_size")]
    pub max_output_size: usize,

    /// Maximum number of output files to decode from the guest VM.
    ///
    /// If the guest sends more files than this limit, decoding will fail.
    /// Defaults to 255.
    #[serde(default = "default_max_file_count")]
    pub max_file_count: u32,

    /// CPU layout for isolating benchmark cores from housekeeping tasks.
    ///
    /// When set, the Firecracker process will be pinned to benchmark cores
    /// via cgroups cpuset. This field is not serialized.
    #[serde(skip)]
    pub cpu_layout: Option<CpuLayout>,

    /// Firecracker process log level. This field is not serialized.
    #[cfg(target_os = "linux")]
    #[serde(skip)]
    pub firecracker_log_level: FirecrackerLogLevel,
}

fn default_vcpus() -> Cpu {
    Cpu::MIN
}

fn default_memory() -> Memory {
    Memory::from_mib(512)
}

fn default_disk() -> Disk {
    Disk::from_mib(1024) // 1 GiB
}

fn default_kernel_cmdline() -> String {
    "console=ttyS0 reboot=t panic=1 pci=off root=/dev/vda rw init=/init".to_owned()
}

const fn default_timeout_secs() -> u64 {
    300 // 5 minutes
}

const fn default_max_output_size() -> usize {
    25 * 1024 * 1024 // 25 MiB
}

const fn default_max_file_count() -> u32 {
    255
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
            memory: default_memory(),
            disk: default_disk(),
            kernel_cmdline: default_kernel_cmdline(),
            timeout_secs: default_timeout_secs(),
            file_paths: None,
            network: false,
            entrypoint: None,
            cmd: None,
            env: None,
            max_output_size: default_max_output_size(),
            max_file_count: default_max_file_count(),
            cpu_layout: None,
            #[cfg(target_os = "linux")]
            firecracker_log_level: FirecrackerLogLevel::default(),
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
            memory: default_memory(),
            disk: default_disk(),
            kernel_cmdline: default_kernel_cmdline(),
            timeout_secs: default_timeout_secs(),
            file_paths: None,
            network: false,
            entrypoint: None,
            cmd: None,
            env: None,
            max_output_size: default_max_output_size(),
            max_file_count: default_max_file_count(),
            cpu_layout: None,
            #[cfg(target_os = "linux")]
            firecracker_log_level: FirecrackerLogLevel::default(),
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
    pub fn with_vcpus(mut self, vcpus: Cpu) -> Self {
        self.vcpus = vcpus;
        self
    }

    /// Set the memory.
    #[must_use]
    pub fn with_memory(mut self, memory: Memory) -> Self {
        self.memory = memory;
        self
    }

    /// Set the disk size.
    #[must_use]
    pub fn with_disk(mut self, disk: Disk) -> Self {
        self.disk = disk;
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

    /// Set the output file paths (inside the guest VM).
    ///
    /// When set, the files will be read by the init process after the
    /// benchmark completes and sent to the host via vsock port 5005
    /// using a length-prefixed binary protocol.
    #[must_use]
    pub fn with_file_paths(mut self, file_paths: Vec<Utf8PathBuf>) -> Self {
        self.file_paths = Some(file_paths);
        self
    }

    /// Set the output file paths from an Option.
    #[must_use]
    pub fn with_file_paths_opt(mut self, file_paths: Option<Vec<Utf8PathBuf>>) -> Self {
        self.file_paths = file_paths;
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

    /// Set the maximum number of output files to decode.
    #[must_use]
    pub fn with_max_file_count(mut self, max_file_count: u32) -> Self {
        self.max_file_count = max_file_count;
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
#[expect(clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn config_new_defaults() {
        let config = Config::new("my-image:latest");
        assert_eq!(config.oci_image, "my-image:latest");
        assert_eq!(config.vcpus, Cpu::MIN);
        assert_eq!(config.memory, Memory::from_mib(512));
        assert_eq!(config.disk, Disk::from_mib(1024));
        assert_eq!(config.timeout_secs, 300);
        assert!(!config.network);
        assert!(config.kernel.is_none());
        assert!(config.token.is_none());
        assert!(config.file_paths.is_none());
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
            .with_vcpus(Cpu::try_from(4).unwrap())
            .with_memory(Memory::from_mib(2048))
            .with_disk(Disk::from_mib(4096))
            .with_timeout_secs(600)
            .with_file_paths(vec![Utf8PathBuf::from("/tmp/results.json")])
            .with_network(true)
            .with_entrypoint(vec!["/bin/sh".to_owned()])
            .with_cmd(vec!["-c".to_owned(), "echo hello".to_owned()])
            .with_env(env.clone());

        assert_eq!(config.token.unwrap(), "jwt-token");
        assert_eq!(config.vcpus, Cpu::try_from(4).unwrap());
        assert_eq!(config.memory, Memory::from_mib(2048));
        assert_eq!(config.disk, Disk::from_mib(4096));
        assert_eq!(config.timeout_secs, 600);
        assert_eq!(
            config.file_paths.unwrap(),
            vec![Utf8PathBuf::from("/tmp/results.json")]
        );
        assert!(config.network);
        assert_eq!(config.entrypoint.unwrap(), vec!["/bin/sh"]);
        assert_eq!(config.cmd.unwrap(), vec!["-c", "echo hello"]);
        assert_eq!(config.env.unwrap(), env);
    }

    #[test]
    fn config_serde_round_trip() {
        let config = Config::new("ghcr.io/test/bench:v1")
            .with_vcpus(Cpu::try_from(2).unwrap())
            .with_memory(Memory::from_mib(1024))
            .with_file_paths(vec![Utf8PathBuf::from("/output.json")]);

        let json = serde_json::to_string(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.oci_image, "ghcr.io/test/bench:v1");
        assert_eq!(parsed.vcpus, Cpu::try_from(2).unwrap());
        assert_eq!(parsed.memory, Memory::from_mib(1024));
        assert_eq!(
            parsed.file_paths.unwrap(),
            vec![Utf8PathBuf::from("/output.json")]
        );
        // Optional None fields should not appear in JSON
        assert!(!json.contains("\"token\""));
        assert!(!json.contains("\"kernel\""));
    }

    #[test]
    fn config_deserialize_with_defaults() {
        let json = r#"{"oci_image": "test:latest"}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.oci_image, "test:latest");
        assert_eq!(config.vcpus, Cpu::MIN);
        assert_eq!(config.memory, Memory::from_mib(512));
        assert_eq!(config.disk, Disk::from_mib(1024));
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
        assert_eq!(&config.env.unwrap()["KEY"], "value");

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
