use url::Url;

use crate::cli::clap::CliOutput;
use crate::BencherError;

const BENCHER_URL: &str = "https://bencher.dev";

#[derive(Debug, Default)]
pub enum Output {
    Headless,
    #[default]
    Web,
    Url(String),
}

impl From<CliOutput> for Output {
    fn from(output: CliOutput) -> Self {
        if output.headless {
            return Self::Headless;
        }
        if output.web {
            return Self::Web;
        }
        if let Some(url) = output.url {
            return Self::Url(url);
        }
        Self::Web
    }
}

impl Output {
    pub fn open(&self, reports: &str) -> Result<(), BencherError> {
        match &self {
            Self::Headless => Ok(println!("{reports}")),
            Self::Web => open_url(BENCHER_URL, reports),
            Self::Url(url) => open_url(url, reports),
        }
    }
}

fn open_url(url: &str, reports: &str) -> Result<(), BencherError> {
    let url = Url::parse_with_params(url, &[("reports", reports)])?;
    open::that(url.as_str()).map_err(|e| e.into())
}
