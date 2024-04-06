use bencher_json::Jwt;
use clap::Parser;
use url::Url;

#[derive(Parser, Debug)]
pub struct TaskEmailList {
    #[clap(long)]
    pub host: Option<Url>,

    #[clap(long)]
    pub token: Option<Jwt>,
}
