use bencher_json::{Jwt, Url};
use clap::Parser;

/// Run runner smoke test (docker pull/push + bencher run --image)
#[derive(Parser, Debug, Default)]
pub struct TaskRunner {
    /// Test API URL
    #[clap(long)]
    pub url: Option<Url>,

    /// Admin token for runner token rotation
    #[clap(long, requires = "with-daemon")]
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
}
