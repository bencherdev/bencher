use bencher_json::{Jwt, Url};
use clap::Parser;

/// Run OCI Distribution Spec conformance tests
#[derive(Parser, Debug, Default)]
pub struct TaskOci {
    /// API URL to test against
    #[clap(long)]
    pub url: Option<Url>,

    /// Repository namespace for tests
    #[clap(long)]
    pub namespace: Option<String>,

    /// Cross-mount namespace for tests
    #[clap(long)]
    pub crossmount_namespace: Option<String>,

    /// Run only pull tests
    #[clap(long)]
    pub pull_only: bool,

    /// Skip building the conformance binary
    #[clap(long)]
    pub skip_build: bool,

    /// Enable debug output from conformance tests
    #[clap(long)]
    pub debug: bool,

    /// Directory for conformance test output
    #[clap(long)]
    pub output_dir: Option<String>,

    /// Path to distribution-spec clone (will clone if not exists)
    #[clap(long)]
    pub spec_dir: Option<String>,

    /// Username for OCI authentication (email address)
    #[clap(long)]
    pub username: Option<String>,

    /// Password for OCI authentication (API token)
    #[clap(long)]
    pub password: Option<Jwt>,
}
