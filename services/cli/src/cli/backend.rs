use std::convert::TryFrom;

use reports::Report;
use url::Url;

use crate::cli::clap::CliBackend;
use crate::cli::BENCHER_URL;
use crate::BencherError;

#[derive(Debug)]
pub struct Backend {
    url: Option<Url>,
}

impl TryFrom<CliBackend> for Backend {
    type Error = BencherError;

    fn try_from(backend: CliBackend) -> Result<Self, Self::Error> {
        Ok(Self {
            url: map_url(backend.url)?,
        })
    }
}

fn map_url(url: Option<String>) -> Result<Option<Url>, url::ParseError> {
    Ok(if let Some(url) = url {
        Some(Url::parse(&url)?)
    } else {
        None
    })
}

impl Backend {
    pub async fn send(&self, report: Report) -> Result<(), BencherError> {
        let url = self
            .url
            .as_ref()
            .map(|url| url.to_string())
            .unwrap_or(format!("{BENCHER_URL}/reports"));

        let client = reqwest::Client::new();
        let res = client.put(&url).json(&report).send().await?;
        println!("{res:?}");
        Ok(())
    }
}
