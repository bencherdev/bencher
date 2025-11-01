use bencher_json::Jwt;
use clap::Parser;
use url::Url;

#[derive(Parser, Debug)]
pub struct TaskEmailList {
    /// Backend host URL
    #[clap(long)]
    pub host: Option<Url>,

    /// User API token
    #[clap(long)]
    pub token: Option<Jwt>,
}
