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

    /// Firecracker major.minor version to pin (e.g. "1.15")
    #[clap(long, default_value = "1.15")]
    pub firecracker_version: String,

    /// Linux kernel major.minor version to pin (e.g. "6.1")
    #[clap(long, default_value = "6.1")]
    pub kernel_version: String,
}
