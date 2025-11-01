use clap::Args;
use url::Url;

#[derive(Args, Debug)]
pub struct TaskLive {
    /// Backend host URL
    #[clap(long)]
    pub host: Option<Url>,
}
