use camino::Utf8PathBuf;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct TaskUpdateSandbox {
    /// Print what would change without modifying files
    #[clap(long)]
    pub dry_run: bool,

    /// Path to the runner build.rs file
    #[clap(long)]
    pub build_rs: Option<Utf8PathBuf>,

    /// Hold the Firecracker pin to one major.minor line (e.g. "1.16"),
    /// instead of taking the newest release of the pinned major.
    /// A new major may change the API, so moving to one is a manual step.
    #[clap(long)]
    pub firecracker_version: Option<String>,

    /// Write the notice about a newer Firecracker major to this path, or
    /// truncate the file to empty when the tracked major leads the releases
    #[clap(long)]
    pub newer_major_out: Option<Utf8PathBuf>,

    /// Linux kernel major.minor version to pin (e.g. "6.1")
    #[clap(long, default_value = "6.1")]
    pub kernel_version: String,
}
