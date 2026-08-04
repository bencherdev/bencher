#[cfg(feature = "plus")]
mod tuning;
#[cfg(feature = "plus")]
mod up;

#[cfg(feature = "plus")]
use bencher_json::Iteration;
#[cfg(feature = "plus")]
use bencher_parser::check_env;
#[cfg(feature = "plus")]
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};

#[cfg(feature = "plus")]
pub use tuning::CliTuning;
#[cfg(feature = "plus")]
pub use up::CliUp;

#[derive(Parser, Debug)]
#[command(name = "runner", version)]
#[command(about = "Execute benchmarks in isolated Firecracker microVMs", long_about = None)]
pub struct CliRunner {
    #[command(subcommand)]
    pub sub: CliSub,
}

#[derive(Subcommand, Debug)]
pub enum CliSub {
    #[cfg(feature = "plus")]
    /// Start the runner, polling for and executing benchmark jobs.
    Up(CliUp),
    #[cfg(feature = "plus")]
    /// Pull image, create rootfs, and execute in isolated Firecracker microVM.
    Run(CliRun),
}

/// Arguments for the `run` subcommand.
#[cfg(feature = "plus")]
#[derive(Parser, Debug)]
pub struct CliRun {
    /// OCI image (local path or registry reference).
    #[arg(long)]
    pub image: String,

    /// JWT token for registry authentication.
    #[arg(long)]
    pub token: Option<String>,

    /// Number of vCPUs (overrides default for testing).
    #[arg(long)]
    pub vcpus: Option<u32>,

    /// Memory in MiB (overrides default for testing).
    #[arg(long)]
    pub memory: Option<u64>,

    /// Disk size in MiB (overrides default for testing).
    #[arg(long)]
    pub disk: Option<u64>,

    /// Execution timeout in seconds.
    #[arg(long, default_value = "300")]
    pub timeout: u64,

    /// Persistent state directory for the runner (absolute path).
    #[arg(
        long,
        env = "BENCHER_STATE_DIR",
        default_value = bencher_runner::DEFAULT_STATE_DIR,
        value_parser = absolute_state_dir,
    )]
    pub state_dir: Utf8PathBuf,

    /// Unprivileged uid the jailed sandbox process drops to.
    #[arg(
        long,
        env = "BENCHER_JAIL_UID",
        default_value_t = bencher_runner::DEFAULT_JAIL_UID,
        value_parser = unprivileged_jail_uid,
    )]
    pub jail_uid: u32,

    /// Unprivileged gid the jailed sandbox process drops to.
    #[arg(
        long,
        env = "BENCHER_JAIL_GID",
        default_value_t = bencher_runner::DEFAULT_JAIL_GID,
        value_parser = unprivileged_jail_gid,
    )]
    pub jail_gid: u32,

    /// Output file paths inside guest (may be repeated).
    #[arg(long)]
    pub output: Vec<Utf8PathBuf>,

    /// Maximum size in bytes for collected stdout/stderr (default: 25 MiB).
    #[arg(long)]
    pub max_output_size: Option<usize>,

    /// Maximum number of output files to decode (default: 255).
    #[arg(long)]
    pub max_file_count: Option<u32>,

    /// Maximum number of symlinks to follow during path resolution (default: 40).
    /// Matches the Linux kernel's MAXSYMLINKS limit. Only used in non-sandboxed mode.
    #[arg(long, conflicts_with = "sandbox")]
    pub max_symlinks: Option<u32>,

    /// Container entrypoint override.
    #[arg(long, num_args = 1..=bencher_json::MAX_ENTRYPOINT_LEN)]
    pub entrypoint: Option<Vec<String>>,

    /// Container command override.
    #[arg(long, num_args = 1..=bencher_json::MAX_CMD_LEN)]
    pub cmd: Option<Vec<String>>,

    /// Environment variable in KEY=VALUE format (may be repeated).
    #[arg(long, value_parser = check_env)]
    pub env: Option<Vec<String>>,

    /// Enable network access in the VM.
    #[arg(long)]
    pub network: bool,

    /// Sandbox mode for benchmark execution.
    /// Use "firecracker" for Firecracker microVM (Linux-only).
    /// Omit for non-sandboxed host execution.
    #[arg(long)]
    pub sandbox: Option<bencher_json::Sandbox>,

    #[command(flatten)]
    pub tuning: CliTuning,

    /// Number of benchmark iterations to execute (default: 1).
    #[arg(long, default_value = "1")]
    pub iter: Iteration,

    /// Allow benchmark failure without short-circuiting iterations.
    #[arg(long)]
    pub allow_failure: bool,

    /// Grace period in seconds after exit code before final collection (default: 1).
    #[arg(long, default_value = "1")]
    pub grace_period: bencher_runner::GracePeriod,

    /// Sandbox process log level; requires --sandbox (default: warning).
    #[arg(long, default_value = "warning", requires = "sandbox")]
    pub sandbox_log_level: bencher_runner::SandboxLogLevel,
}

/// Require an absolute state directory, before the runner starts.
///
/// The rule itself lives in the library, on the type that holds the path, since
/// every caller that hands it a state directory is exposed to the same thing.
/// This is the same check run early, so an operator hears about it at the
/// command line rather than when the first sandboxed Job builds its jail.
#[cfg(feature = "plus")]
fn absolute_state_dir(arg: &str) -> Result<Utf8PathBuf, String> {
    let path = Utf8PathBuf::from(arg);
    bencher_runner::check_absolute_state_dir(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Require an unprivileged jail uid, before the runner starts.
///
/// The same arrangement as [`absolute_state_dir`], and for a second reason
/// besides having one implementation of the rule. A `range(1..)` value parser
/// rejects `0` just as reliably and answers `0 is not in 1..`, so the sentence
/// [`bencher_runner::JailUser`] carries, that the sandbox is built by dropping
/// privilege and a jail user of root is no jail at all, was reachable only by a
/// library caller. The operator most likely to try `--jail-uid 0` is the one
/// staring at a permission error, and is exactly who that sentence is for.
///
/// The environment is covered by the same parser: clap runs it on
/// `BENCHER_JAIL_UID` as well as on the flag.
#[cfg(feature = "plus")]
fn unprivileged_jail_uid(arg: &str) -> Result<u32, String> {
    let uid = arg
        .parse()
        .map_err(|e: std::num::ParseIntError| e.to_string())?;
    // Checked against the default gid, which is not root, so the library
    // answers about the field being parsed rather than about its partner.
    bencher_runner::JailUser::new(uid, bencher_runner::DEFAULT_JAIL_GID)
        .map_err(|e| e.to_string())?;
    Ok(uid)
}

/// Require an unprivileged jail gid, before the runner starts.
///
/// See [`unprivileged_jail_uid`].
#[cfg(feature = "plus")]
fn unprivileged_jail_gid(arg: &str) -> Result<u32, String> {
    let gid = arg
        .parse()
        .map_err(|e: std::num::ParseIntError| e.to_string())?;
    bencher_runner::JailUser::new(bencher_runner::DEFAULT_JAIL_UID, gid)
        .map_err(|e| e.to_string())?;
    Ok(gid)
}

#[cfg(all(test, feature = "plus"))]
mod tests {
    use super::{absolute_state_dir, unprivileged_jail_gid, unprivileged_jail_uid};

    #[test]
    fn a_relative_state_dir_is_refused() {
        // A relative path resolves against the jailer's working directory, not
        // the runner's, so it has to be caught before it can be handed over.
        assert_eq!(
            absolute_state_dir("/var/lib/bencher-runner").unwrap(),
            "/var/lib/bencher-runner"
        );
        absolute_state_dir("bencher-runner").unwrap_err();
        absolute_state_dir("./bencher-runner").unwrap_err();
        absolute_state_dir("").unwrap_err();
    }

    #[test]
    fn a_root_jail_user_is_refused_with_the_library_reason() {
        // A `range(1..)` value parser refuses 0 just as well and says "0 is not
        // in 1..". What the operator has to read is why, and only the library's
        // own check says it.
        assert_eq!(unprivileged_jail_uid("61016").unwrap(), 61016);
        assert_eq!(unprivileged_jail_gid("61016").unwrap(), 61016);

        let uid = unprivileged_jail_uid("0").unwrap_err();
        assert!(uid.contains("uid"), "names the field it refused: {uid}");
        assert!(uid.contains("no jail at all"), "says why: {uid}");

        let gid = unprivileged_jail_gid("0").unwrap_err();
        assert!(gid.contains("gid"), "names the field it refused: {gid}");
        assert!(gid.contains("no jail at all"), "says why: {gid}");
    }

    #[test]
    fn a_jail_user_that_is_not_a_uid_is_refused() {
        // The library never sees these: an id it cannot be given is the
        // argument parser's own business.
        unprivileged_jail_uid("-1").unwrap_err();
        unprivileged_jail_uid("").unwrap_err();
        unprivileged_jail_uid("root").unwrap_err();
        unprivileged_jail_gid("-1").unwrap_err();
    }
}
