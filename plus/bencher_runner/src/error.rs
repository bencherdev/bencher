use camino::Utf8PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("OCI error: {0}")]
    Oci(#[from] bencher_oci::OciError),

    #[error("Rootfs error: {0}")]
    Rootfs(#[from] bencher_rootfs::RootfsError),

    #[cfg(target_os = "linux")]
    #[error("Firecracker error: {0}")]
    Firecracker(#[from] crate::firecracker::FirecrackerError),

    #[error("Jail error: {0}")]
    Jail(#[from] JailError),

    #[error("Config error: {0}")]
    Config(#[from] ConfigError),

    #[error("Execution error: {0}")]
    Execution(#[from] ExecutionError),

    #[error("Benchmark exited with non-zero exit code: {0}")]
    NonZeroExitCode(i32),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum JailError {
    #[error("Failed to create cgroup {path}: {source}")]
    CreateCgroup {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to enable cgroup controllers at {path}: {source}")]
    EnableControllers {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[error("Required cgroup controller '{controller}' not enabled at {path}. Enabled: {enabled}")]
    MissingController {
        controller: String,
        path: Utf8PathBuf,
        enabled: String,
    },

    #[error("Failed to write cgroup file {path}: {source}")]
    WriteCgroup {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[error("Cpuset partition mode '{mode}' rejected by the kernel: {state}")]
    PartitionInvalid { mode: String, state: String },

    #[error("Failed to create runner state directory {path}: {source}")]
    CreateStateDir {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[cfg(target_os = "linux")]
    #[error("Failed to create network namespace directory {path}: {source}")]
    NetnsDir {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[cfg(target_os = "linux")]
    #[error("Failed to create network namespace handle {path}: {source}")]
    NetnsHandle {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[cfg(target_os = "linux")]
    #[error("Failed to unshare the network namespace: {0}")]
    Unshare(#[source] nix::Error),

    #[cfg(target_os = "linux")]
    #[error("Failed to bind the network namespace handle {path}: {source}")]
    BindNetns {
        path: Utf8PathBuf,
        source: nix::Error,
    },

    #[cfg(target_os = "linux")]
    #[error("The network namespace thread panicked")]
    NetnsThread,

    #[cfg(target_os = "linux")]
    #[error("Failed to create jail chroot {path}: {source}")]
    CreateJail {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[cfg(target_os = "linux")]
    #[error("Failed to hand {path} to the jail uid and gid: {source}")]
    ChownJail {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[cfg(target_os = "linux")]
    #[error("Failed to open {path} for cgroup placement: {source}")]
    OpenCgroupProcs {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[cfg(target_os = "linux")]
    #[error("Failed to read cgroup file {path}: {source}")]
    ReadCgroup {
        path: Utf8PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Unsupported platform: {0}")]
    UnsupportedPlatform(String),

    #[error("Failed to create temp directory: {0}")]
    TempDir(#[source] std::io::Error),

    #[error("Temp directory path is not UTF-8")]
    NonUtf8TempDir,

    #[error("OCI image has no CMD or ENTRYPOINT set")]
    MissingCommand,

    #[error("Binary not found: {name}. {hint}")]
    BinaryNotFound { name: String, hint: String },

    #[error("Failed to copy {src} to {dest}: {source}")]
    CopyFile {
        src: Utf8PathBuf,
        dest: Utf8PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to copy init binary from {src} to {dest}: {source}")]
    CopyInit {
        src: Utf8PathBuf,
        dest: Utf8PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to serialize config: {0}")]
    Serialize(#[source] serde_json::Error),

    #[error("{name} {value} out of range ({range})")]
    OutOfRange {
        name: &'static str,
        value: String,
        range: &'static str,
    },

    #[error("Invalid registry token: {0}")]
    InvalidToken(#[from] bencher_valid::ValidError),
}

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("Benchmark timeout: {0}")]
    Timeout(String),

    #[error("Benchmark canceled: {0}")]
    Canceled(String),

    #[error("Benchmark setup: {0}")]
    Setup(String),
}
