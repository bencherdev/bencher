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

    #[error(
        "The jail {field} must not be 0: the sandbox is built by dropping privilege, so a jail user of root is no jail at all"
    )]
    PrivilegedJailUser { field: &'static str },

    #[error(
        "The state directory {path} must be an absolute path. It reaches the jailer as --chroot-base-dir, which the jailer resolves against its own working directory rather than the runner's, so a relative path builds the chroot somewhere the runner does not look."
    )]
    RelativeStateDir { path: Utf8PathBuf },

    #[error(
        "The state directory {path} already exists, is not empty, and was not created by the runner. Point --state-dir at a directory the runner owns, or at a subdirectory of this one."
    )]
    ForeignStateDir { path: Utf8PathBuf },

    #[error("Failed to create runner state directory {path}: {source}")]
    CreateStateDir {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[error(
        "Failed to read the runner state directory {path}: {source}. The runner cannot tell whether this directory is its own, and it will not tighten the permissions of one that might not be."
    )]
    ReadStateDir {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[error(
        "Failed to read the jail directory {path}: {source}. The sweep cannot tell what a previous runner left behind, and a directory it could not read is not an empty one."
    )]
    ReadJailParent {
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
    #[error(
        "A Runner executing sandboxed Jobs must run as root, but this one is running as uid {euid}. \
         Building the sandbox needs privileges that enabling KVM does not grant: mknod for the chroot's /dev/kvm, \
         chown to hand the guest images to the sandbox user, pivot_root, and setns to join a network namespace. \
         A world-readable /dev/kvm is enough to use KVM unprivileged but not to build the sandbox around it. \
         Run as root, or give up the sandbox: start `runner up` with --danger-allow-no-sandbox and assign it \
         only Specs with no Sandbox, or invoke `runner run` without --sandbox. Either way what is given up is \
         the microVM itself and not just its confinement, so the Job executes directly on the host."
    )]
    NotRoot { euid: u32 },

    #[cfg(target_os = "linux")]
    #[error("Failed to open the jail lock {path}: {source}")]
    OpenJailLock {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[cfg(target_os = "linux")]
    #[error("Failed to take the jail lock {path}: {source}")]
    JailLock {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[cfg(target_os = "linux")]
    #[error("Failed to open the network namespace lock {path}: {source}")]
    OpenNetnsLock {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[cfg(target_os = "linux")]
    #[error("Failed to take the network namespace lock {path}: {source}")]
    NetnsLock {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[cfg(target_os = "linux")]
    #[error(
        "The network namespace handle {path} is not a namespace distinct from the runner's own"
    )]
    NetnsNotDistinct { path: Utf8PathBuf },

    #[cfg(target_os = "linux")]
    #[error(
        "A stale jail at {path} still has VMM pid {pid} running in it. It is executing untrusted guest code on the benchmark cores, so any measurement taken now is contended. The jail is left in place until it can be reaped."
    )]
    JailStillRunning { path: Utf8PathBuf, pid: u32 },

    #[cfg(target_os = "linux")]
    #[error(
        "A stale jail at {path} could not be examined, so whether a VMM is still running in it is unknown. It is left in place, and a jail that cannot be checked is not a jail that has been cleared."
    )]
    JailUnexaminable { path: Utf8PathBuf },

    #[cfg(target_os = "linux")]
    #[error(
        "The kernel narrowed the cgroup cpuset at {path}: asked for cpus {requested}, got {effective}. The benchmark would not have run on the cores it claims."
    )]
    CpusetNarrowed {
        path: Utf8PathBuf,
        requested: String,
        effective: String,
    },

    #[cfg(target_os = "linux")]
    #[error(
        "Failed to remove the stale cgroup {path}: {source}. Stale cgroups accumulate under the parent, and one that cannot be removed usually means something is still running in it."
    )]
    StaleCgroup {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[cfg(target_os = "linux")]
    #[error("Failed to open the jail chroot {path}: {source}")]
    OpenJailRoot {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[cfg(target_os = "linux")]
    #[error(
        "The socket path {path} is {length} bytes, over the {limit} byte sun_path limit for a Unix domain socket"
    )]
    SocketPathTooLong {
        path: String,
        length: usize,
        limit: usize,
    },

    #[cfg(target_os = "linux")]
    #[error("Failed to create jail chroot {path}: {source}")]
    CreateJail {
        path: Utf8PathBuf,
        source: std::io::Error,
    },

    #[cfg(target_os = "linux")]
    #[error("Failed to make {path} readable by the jailed VMM: {source}")]
    ChmodJail {
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
