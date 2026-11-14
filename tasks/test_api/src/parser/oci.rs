use std::path::PathBuf;

use bencher_json::{Url, UserKey};
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

    /// Password for OCI authentication (user API key), or one the admin creates when unset
    #[clap(long)]
    pub password: Option<UserKey>,

    /// The API's log, where a server that sends no email writes each sign-in email,
    /// to make the admin's key from a browser session signed in by its code
    #[clap(long)]
    pub api_log: Option<PathBuf>,
}
