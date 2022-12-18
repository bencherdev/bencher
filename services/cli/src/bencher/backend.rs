use std::convert::TryFrom;

use bencher_json::Jwt;
use serde::Serialize;
use tokio::time::{sleep, Duration};
use url::Url;

use crate::{cli::CliBackend, cli_println, CliError};

pub const BENCHER_API_TOKEN: &str = "BENCHER_API_TOKEN";
pub const BENCHER_HOST: &str = "BENCHER_HOST";
#[cfg(debug_assertions)]
pub const DEFAULT_HOST: &str = "http://localhost:61016";
#[cfg(not(debug_assertions))]
pub const DEFAULT_HOST: &str = "https://api.bencher.dev";
const DEFAULT_ATTEMPTS: usize = 3;
const DEFAULT_SLEEP: u64 = 1;

#[derive(Debug, Clone)]
pub struct Backend {
    pub host: Url,
    pub token: Option<Jwt>,
    pub attempts: Option<usize>,
    pub sleep: Option<u64>,
}

impl TryFrom<CliBackend> for Backend {
    type Error = CliError;

    fn try_from(backend: CliBackend) -> Result<Self, Self::Error> {
        Ok(Self {
            host: unwrap_host(backend.host)?,
            token: map_token(backend.token)?,
            attempts: backend.attempts,
            sleep: backend.sleep,
        })
    }
}

fn unwrap_host(host: Option<String>) -> Result<Url, CliError> {
    if let Some(url) = host {
        url
    } else if let Ok(url) = std::env::var(BENCHER_HOST) {
        url
    } else {
        DEFAULT_HOST.into()
    }
    .parse()
    .map_err(Into::into)
}

fn map_token(token: Option<String>) -> Result<Option<Jwt>, CliError> {
    Ok(if let Some(token) = token {
        Some(token.parse()?)
    } else if let Ok(token) = std::env::var(BENCHER_API_TOKEN) {
        Some(token.parse()?)
    } else {
        None
    })
}

impl Backend {
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

        let attempts = self.attempts.unwrap_or(DEFAULT_ATTEMPTS);
        let sleep_secs = self.sleep.unwrap_or(DEFAULT_SLEEP);
        for attempt in 0..attempts {
            match builder
                .try_clone()
                .ok_or(CliError::CloneBackend)?
                .send()
                .await
            {
                Ok(res) => {
                    let json = res.json().await?;
                    cli_println!("{}", serde_json::to_string_pretty(&json)?);
                    return Ok(json);
                },
                Err(e) => {
                    cli_println!("Send attempt #{attempt}: {e}");
                    if attempt != attempts - 1 {
                        cli_println!("Will attempt to send again in {sleep_secs} second(s).");
                        sleep(Duration::from_secs(sleep_secs)).await;
                    }
                },
            }
        }

        Err(CliError::Send(attempts))
    }
}

enum Method<T> {
    Get,
    Post(T),
    Put(T),
    Patch(T),
    Delete,
}
