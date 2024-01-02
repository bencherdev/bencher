use clap::Parser;
use url::Url;

#[derive(Parser, Debug)]
pub struct CliNotify {
    pub message: String,

    #[clap(long)]
    pub topic: Option<String>,
    #[clap(long)]
    pub title: Option<String>,
    #[clap(long)]
    pub tag: Option<String>,
    #[clap(long)]
    pub priority: Option<u8>,
    #[clap(long)]
    pub click: Option<Url>,
    #[clap(long)]
    pub attach: Option<Url>,
}
