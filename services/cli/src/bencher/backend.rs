use std::convert::TryFrom;

use serde::Serialize;
use url::Url;

use crate::{cli::CliBackend, CliError};

pub const BENCHER_API_TOKEN: &str = "BENCHER_API_TOKEN";
pub const BENCHER_URL: &str = "BENCHER_URL";
pub const BENCHER_HOST: &str = "BENCHER_HOST";
pub const DEFAULT_URL: &str = "https://api.bencher.dev";

#[derive(Debug, Clone)]
pub struct Backend {
    pub token: Option<String>,
    pub host: Url,
}

impl TryFrom<CliBackend> for Backend {
    type Error = CliError;

    fn try_from(backend: CliBackend) -> Result<Self, Self::Error> {
        Ok(Self {
            token: map_token(backend.token)?,
            host: unwrap_host(backend.host)?,
        })
    }
}

fn map_token(token: Option<String>) -> Result<Option<String>, CliError> {
    // TODO add first pass token validation
    if let Some(token) = token {
        Ok(Some(token))
    } else if let Ok(token) = std::env::var(BENCHER_API_TOKEN) {
        Ok(Some(token))
    } else {
        Err(CliError::TokenNotFound)
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
    Url::parse(&url)
}

impl Backend {
    pub fn new(token: Option<String>, host: Option<String>) -> Result<Self, CliError> {
        Ok(Self {
            token,
            host: unwrap_host(host)?,
        })
    }

    pub async fn get(&self, path: &str) -> Result<serde_json::Value, CliError> {
        self.send::<()>(Method::Get, path).await
    }

    pub async fn post<T>(&self, path: &str, json: &T) -> Result<serde_json::Value, CliError>
    where
        T: Serialize + ?Sized,
    {
        self.send(Method::Post(json), path).await
    }

    pub async fn put<T>(&self, path: &str, json: &T) -> Result<serde_json::Value, CliError>
    where
        T: Serialize + ?Sized,
    {
        self.send(Method::Put(json), path).await
    }

    pub async fn patch<T>(&self, path: &str, json: &T) -> Result<serde_json::Value, CliError>
    where
        T: Serialize + ?Sized,
    {
        self.send(Method::Patch(json), path).await
    }

    pub async fn delete(&self, path: &str) -> Result<serde_json::Value, CliError> {
        self.send::<()>(Method::Delete, path).await
    }

    async fn send<T>(&self, method: Method<&T>, path: &str) -> Result<serde_json::Value, CliError>
    where
        T: Serialize + ?Sized,
    {
        let client = reqwest::Client::new();
        let url = self.host.join(path)?.to_string();
        let mut builder = match method {
            Method::Get => client.get(&url),
            Method::Post(json) => client.post(&url).json(json),
            Method::Put(json) => client.put(&url).json(json),
            Method::Patch(json) => client.patch(&url).json(json),
            Method::Delete => client.delete(&url),
        };
        if let Some(token) = &self.token {
            builder = builder.header("Authorization", format!("Bearer {token}"));
        }
        let res: serde_json::Value = builder.send().await?.json().await?;
        println!("{}", serde_json::to_string_pretty(&res)?);
        Ok(res)
    }
}

enum Method<T> {
    Get,
    Post(T),
    Put(T),
    Patch(T),
    Delete,
}
