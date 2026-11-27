use std::path::PathBuf;

use bencher_json::Url;
use clap::Parser;

/// Run runner smoke test (docker pull/push + bencher run --image)
#[derive(Parser, Debug, Default)]
pub struct TaskRunner {
    /// Test API URL
    #[clap(long)]
    pub url: Option<Url>,

    /// Username for OCI authentication (email address)
    #[clap(long)]
    pub username: Option<String>,

    /// Start a runner daemon locally for the test
    #[clap(long)]
    pub with_daemon: bool,

    /// The API's log, where a server that sends no email writes each sign-in email,
    /// to sign the seed's users in by its code and make keys from their sessions.
    /// Dev's API is read from its Fly.io logs.
    #[clap(long)]
    pub api_log: Option<PathBuf>,
}
