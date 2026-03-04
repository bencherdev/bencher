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
}
