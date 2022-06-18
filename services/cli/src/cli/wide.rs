use std::convert::TryFrom;

use email_address_parser::EmailAddress;
use reports::Report;
use url::Url;

use crate::cli::clap::CliWide;
use crate::BencherError;

pub const BENCHER_EMAIL: &str = "BENCHER_EMAIL";
pub const BENCHER_TOKEN: &str = "BENCHER_TOKEN";
pub const BENCHER_URL: &str = "https://api.bencher.dev";

#[derive(Debug)]
pub struct Wide {
    pub email: EmailAddress,
    pub token: String,
    pub url: Option<Url>,
}

impl TryFrom<CliWide> for Wide {
    type Error = BencherError;

    fn try_from(wide: CliWide) -> Result<Self, Self::Error> {
        Ok(Self {
            email: map_email(wide.email)?,
            token: map_token(wide.token)?,
            url: map_url(wide.url)?,
        })
    }
}

fn map_email(email: Option<String>) -> Result<EmailAddress, BencherError> {
    // TODO add first pass token validation
    if let Some(email) = email {
        return map_email_str(email);
    }
    if let Ok(email) = std::env::var(BENCHER_EMAIL) {
        return map_email_str(email);
    };
    Err(BencherError::EmailNotFound)
}

fn map_email_str(email: String) -> Result<EmailAddress, BencherError> {
    EmailAddress::parse(&email, None).ok_or(BencherError::Email(email))
}

fn map_token(token: Option<String>) -> Result<String, BencherError> {
    // TODO add first pass token validation
    if let Some(token) = token {
        return Ok(token);
    }
    if let Ok(token) = std::env::var(BENCHER_TOKEN) {
        return Ok(token);
    };
    Err(BencherError::TokenNotFound)
}

fn map_url(url: Option<String>) -> Result<Option<Url>, url::ParseError> {
    Ok(if let Some(url) = url {
        Some(Url::parse(&url)?)
    } else {
        None
    })
}

impl Wide {
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
