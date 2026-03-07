use bencher_json::{Jwt, Url};
use clap::Parser;

/// Run runner smoke test (docker pull/push + bencher run --image)
#[derive(Parser, Debug)]
pub struct TaskRunner {
    /// Test API URL
    #[clap(long)]
    pub url: Option<Url>,
    /// Test token (Muriel Bagge)
    #[clap(long)]
    pub token: Option<Jwt>,
    /// OCI registry login username
    #[clap(long)]
    pub username: Option<String>,
    /// Admin token for runner token rotation (required with --with-daemon)
    #[clap(long)]
    pub admin_token: Option<Jwt>,
    /// Start a runner daemon locally for the test
    #[clap(long)]
    pub with_daemon: bool,
}
