//! Jail configuration.

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

/// Configuration for the jail environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JailConfig {
    /// Unique identifier for this jail (used for cgroup naming).
    pub id: String,

    /// Path to the executable to run inside the jail.
    pub exec_path: Utf8PathBuf,

    /// Arguments to pass to the executable.
    #[serde(default)]
    pub exec_args: Vec<String>,

    /// Root directory for the jail (will be `pivot_root` target).
    /// The jailer will create this directory if it doesn't exist.
    pub jail_root: Utf8PathBuf,

    /// User ID to run as inside the jail.
    #[serde(default = "default_uid")]
    pub uid: u32,

    /// Group ID to run as inside the jail.
    #[serde(default = "default_gid")]
    pub gid: u32,

    /// Resource limits.
    #[serde(default)]
    pub limits: ResourceLimits,

    /// Namespaces to create.
    #[serde(default)]
    pub namespaces: NamespaceConfig,

    /// Paths to bind mount into the jail (read-only).
    #[serde(default)]
    pub bind_mounts: Vec<BindMount>,

    /// Working directory inside the jail.
    #[serde(default = "default_workdir")]
    pub workdir: Utf8PathBuf,

    /// Environment variables to set.
    #[serde(default)]
    pub env: Vec<(String, String)>,
}

const fn default_uid() -> u32 {
    0xFFFE // nobody (65534)
}

const fn default_gid() -> u32 {
    0xFFFE // nogroup (65534)
}

fn default_workdir() -> Utf8PathBuf {
    Utf8PathBuf::from("/")
}

/// Resource limits for the jailed process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum CPU time in microseconds per second (cgroup cpu.max).
    /// E.g., 100000 = 100ms per 100ms period = 1 full CPU.
    /// None means unlimited.
    #[serde(default)]
    pub cpu_quota_us: Option<u64>,

    /// CPU period in microseconds (default: 100000 = 100ms).
    #[serde(default = "default_cpu_period")]
    pub cpu_period_us: u64,

    /// Maximum memory in bytes (cgroup memory.max).
    /// None means unlimited.
    #[serde(default)]
    pub memory_bytes: Option<u64>,

    /// Maximum number of open file descriptors (`RLIMIT_NOFILE`).
    #[serde(default = "default_max_fds")]
    pub max_fds: u64,

    /// Maximum file size in bytes (`RLIMIT_FSIZE`).
    /// None means unlimited.
    #[serde(default)]
    pub max_file_size: Option<u64>,

    /// Maximum number of processes/threads (`RLIMIT_NPROC`).
    #[serde(default = "default_max_procs")]
    pub max_procs: u64,
}

const fn default_cpu_period() -> u64 {
    100_000 // 100ms
}

const fn default_max_fds() -> u64 {
    1024
}

const fn default_max_procs() -> u64 {
    64
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpu_quota_us: None,
            cpu_period_us: default_cpu_period(),
            memory_bytes: None,
            max_fds: default_max_fds(),
            max_file_size: None,
            max_procs: default_max_procs(),
        }
    }
}

/// Namespace configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[expect(clippy::struct_excessive_bools)]
pub struct NamespaceConfig {
    /// Create new user namespace.
    #[serde(default = "default_true")]
    pub user: bool,

    /// Create new PID namespace.
    #[serde(default = "default_true")]
    pub pid: bool,

    /// Create new mount namespace.
    #[serde(default = "default_true")]
    pub mount: bool,

    /// Create new network namespace (isolated, no network access).
    #[serde(default = "default_true")]
    pub network: bool,

    /// Create new UTS namespace (hostname isolation).
    #[serde(default = "default_true")]
    pub uts: bool,

    /// Create new IPC namespace.
    #[serde(default = "default_true")]
    pub ipc: bool,

    /// Create new cgroup namespace.
    #[serde(default = "default_true")]
    pub cgroup: bool,
}

const fn default_true() -> bool {
    true
}

impl Default for NamespaceConfig {
    fn default() -> Self {
        Self {
            user: true,
            pid: true,
            mount: true,
            network: true,
            uts: true,
            ipc: true,
            cgroup: true,
        }
    }
}

/// A bind mount specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindMount {
    /// Source path on the host.
    pub source: Utf8PathBuf,

    /// Destination path inside the jail.
    pub dest: Utf8PathBuf,

    /// Whether the mount should be read-only.
    #[serde(default = "default_true")]
    pub read_only: bool,
}

impl JailConfig {
    /// Create a new jail configuration with sensible defaults.
    pub fn new<S: Into<String>>(id: S, exec_path: Utf8PathBuf, jail_root: Utf8PathBuf) -> Self {
        Self {
            id: id.into(),
            exec_path,
            exec_args: Vec::new(),
            jail_root,
            uid: default_uid(),
            gid: default_gid(),
            limits: ResourceLimits::default(),
            namespaces: NamespaceConfig::default(),
            bind_mounts: Vec::new(),
            workdir: default_workdir(),
            env: Vec::new(),
        }
    }

    /// Set the arguments for the executable.
    #[must_use]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.exec_args = args;
        self
    }

    /// Set the UID/GID to run as.
    #[must_use]
    pub fn with_uid_gid(mut self, uid: u32, gid: u32) -> Self {
        self.uid = uid;
        self.gid = gid;
        self
    }

    /// Set memory limit.
    #[must_use]
    pub fn with_memory_limit(mut self, bytes: u64) -> Self {
        self.limits.memory_bytes = Some(bytes);
        self
    }

    /// Set CPU limit (as a fraction of one CPU, e.g., 0.5 = half a CPU).
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    pub fn with_cpu_limit(mut self, cpus: f64) -> Self {
        // cpu_period_us is small enough that f64 precision loss is negligible
        let quota = (cpus * self.limits.cpu_period_us as f64) as u64;
        self.limits.cpu_quota_us = Some(quota);
        self
    }

    /// Add a read-only bind mount.
    #[must_use]
    pub fn with_bind_mount(mut self, source: Utf8PathBuf, dest: Utf8PathBuf) -> Self {
        self.bind_mounts.push(BindMount {
            source,
            dest,
            read_only: true,
        });
        self
    }

    /// Add an environment variable.
    #[must_use]
    pub fn with_env<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Set the working directory inside the jail.
    #[must_use]
    pub fn with_workdir(mut self, workdir: Utf8PathBuf) -> Self {
        self.workdir = workdir;
        self
    }
}
