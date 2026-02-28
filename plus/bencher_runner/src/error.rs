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
}
