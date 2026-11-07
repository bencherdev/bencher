use std::path::PathBuf;

use bencher_json::{Jwt, Url};
use clap::Parser;

/// Run runner smoke test (docker pull/push + bencher run --image)
#[derive(Parser, Debug, Default)]
pub struct TaskRunner {
    /// Test API URL
    #[clap(long)]
    pub url: Option<Url>,

    /// Admin token for runner key rotation
    #[clap(long, requires = "with_daemon")]
    pub admin_token: Option<Jwt>,

    /// Username for OCI authentication (email address)
    #[clap(long)]
    pub username: Option<String>,

    /// User token and password for OCI authentication
    #[clap(long)]
    pub token: Option<Jwt>,

    /// Start a runner daemon locally for the test
    #[clap(long)]
    pub with_daemon: bool,

    /// The API's log, where a server that sends no email writes each sign-in email,
    /// to make keys from a browser session signed in by its code rather than by the tokens
    #[clap(long)]
    pub api_log: Option<PathBuf>,
}
