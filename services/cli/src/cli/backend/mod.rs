use std::convert::TryFrom;

use email_address_parser::EmailAddress;
use url::{Host, Position, Url};

use crate::cli::clap::CliBackend;

use reports::Report;

use crate::cli::BENCHER_URL;
use crate::BencherError;

mod testbed;

use testbed::Testbed;

#[derive(Debug)]
pub struct Backend {
    url: Option<Url>,
    email: EmailAddress,
    project: Option<String>,
    testbed: Testbed,
}

impl TryFrom<CliBackend> for Backend {
    type Error = BencherError;

    fn try_from(backend: CliBackend) -> Result<Self, Self::Error> {
        Ok(Self {
            url: map_url(backend.url)?,
            email: map_email(backend.email)?,
            project: backend.project,
            testbed: Testbed::from(backend.testbed),
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

fn map_email(email: String) -> Result<EmailAddress, BencherError> {
    EmailAddress::parse(&email, None).ok_or(BencherError::Email(email))
}

impl Backend {
    pub async fn send(&self, report: Report) -> Result<(), BencherError> {
        let url = self
            .url
            .as_ref()
            .map(|url| url.to_string())
            .unwrap_or(BENCHER_URL.into());

        let client = reqwest::Client::new();
        let res = client.put(&url).json(&report).send().await?;
        println!("{res:?}");
        Ok(())
    }
}
