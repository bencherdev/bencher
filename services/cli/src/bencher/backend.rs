use std::convert::TryFrom;

use serde::Serialize;
use url::Url;

use crate::{
    cli::CliBackend,
    BencherError,
};

pub const BENCHER_TOKEN: &str = "BENCHER_TOKEN";
pub const BENCHER_URL: &str = "BENCHER_URL";
pub const BENCHER_HOST: &str = "BENCHER_HOST";
pub const DEFAULT_URL: &str = "https://api.bencher.dev";

#[derive(Debug)]
pub struct Backend {
    pub token: Option<String>,
    pub host:  Url,
}

impl TryFrom<CliBackend> for Backend {
    type Error = BencherError;

    fn try_from(backend: CliBackend) -> Result<Self, Self::Error> {
        Ok(Self {
            token: map_token(backend.token)?,
            host:  unwrap_host(backend.host)?,
        })
    }
}

fn map_token(token: Option<String>) -> Result<Option<String>, BencherError> {
    // TODO add first pass token validation
    if let Some(token) = token {
        Ok(Some(token))
    } else if let Ok(token) = std::env::var(BENCHER_TOKEN) {
        Ok(Some(token))
    } else {
        Err(BencherError::TokenNotFound)
    }
}

fn unwrap_host(host: Option<String>) -> Result<Url, url::ParseError> {
    let url = if let Some(url) = host {
        url
    } else if let Ok(url) = std::env::var(BENCHER_URL) {
        url
    } else if let Ok(url) = std::env::var(BENCHER_HOST) {
        url
    } else {
        DEFAULT_URL.into()
    };
    Ok(Url::parse(&url)?)
}

impl Backend {
    pub fn new(token: Option<String>, host: Option<String>) -> Result<Self, BencherError> {
        Ok(Self {
            token,
            host: unwrap_host(host)?,
        })
    }

    pub async fn post<T>(&self, path: &str, json: &T) -> Result<serde_json::Value, BencherError>
    where
        T: Serialize + ?Sized,
    {
        let client = reqwest::Client::new();
        let url = self.host.join(path)?.to_string();
        let mut builder = client.post(&url);
        if let Some(token) = &self.token {
            builder = builder.header("Authorization", format!("Bearer {token}"));
        }
        let res: serde_json::Value = builder.json(json).send().await?.json().await?;
        println!("{}", serde_json::to_string(&res)?);
        Ok(res)
    }
}
