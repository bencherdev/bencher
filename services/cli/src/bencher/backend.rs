use std::convert::TryFrom;

use bencher_json::{Jwt, Url};
use reqwest::{Client, RequestBuilder};
use serde::Serialize;
use tokio::time::{sleep, Duration};

use crate::{cli::CliBackend, cli_println, CliError};

pub const BENCHER_API_TOKEN: &str = "BENCHER_API_TOKEN";
pub const BENCHER_HOST: &str = "BENCHER_HOST";
#[cfg(debug_assertions)]
pub const DEFAULT_HOST: &str = "http://localhost:61016";
#[cfg(not(debug_assertions))]
pub const DEFAULT_HOST: &str = "https://api.bencher.dev";
const DEFAULT_ATTEMPTS: usize = 10;
const DEFAULT_RETRY_AFTER: u64 = 3;

#[derive(Debug, Clone)]
pub struct Backend {
    pub host: url::Url,
    pub token: Option<Jwt>,
    pub attempts: Option<usize>,
    pub retry_after: Option<u64>,
}

impl TryFrom<CliBackend> for Backend {
    type Error = CliError;

    fn try_from(backend: CliBackend) -> Result<Self, Self::Error> {
        Ok(Self {
            host: unwrap_host(backend.host)?,
            token: map_token(backend.token)?,
            attempts: backend.attempts,
            retry_after: backend.retry_after,
        })
    }
}

fn unwrap_host(host: Option<Url>) -> Result<url::Url, CliError> {
    if let Some(url) = host {
        url.into()
    } else if let Ok(env_url) = std::env::var(BENCHER_HOST) {
        env_url
    } else {
        DEFAULT_HOST.into()
    }
    .parse()
    .map_err(Into::into)
}

fn map_token(token: Option<Jwt>) -> Result<Option<Jwt>, CliError> {
    Ok(if let Some(token) = token {
        Some(token)
    } else if let Ok(env_token) = std::env::var(BENCHER_API_TOKEN) {
        Some(env_token.parse()?)
    } else {
        None
    })
}

impl Backend {
    pub async fn get(&self, path: &str) -> Result<serde_json::Value, CliError> {
        self.send::<()>(Method::Get, path, true).await
    }

    pub async fn get_quiet(&self, path: &str) -> Result<serde_json::Value, CliError> {
        self.send::<()>(Method::Get, path, false).await
    }

    pub async fn get_query<T: Serialize + ?Sized>(
        &self,
        path: &str,
        query: &T,
    ) -> Result<serde_json::Value, CliError> {
        self.send(Method::GetQuery(query), path, true).await
    }

    pub async fn get_query_quiet<T: Serialize + ?Sized>(
        &self,
        path: &str,
        query: &T,
    ) -> Result<serde_json::Value, CliError> {
        self.send(Method::GetQuery(query), path, false).await
    }

    pub async fn post<T>(&self, path: &str, json: &T) -> Result<serde_json::Value, CliError>
    where
        T: Serialize + ?Sized,
    {
        self.send(Method::Post(json), path, true).await
    }

    pub async fn put<T>(&self, path: &str, json: &T) -> Result<serde_json::Value, CliError>
    where
        T: Serialize + ?Sized,
    {
        self.send(Method::Put(json), path, true).await
    }

    pub async fn patch<T>(&self, path: &str, json: &T) -> Result<serde_json::Value, CliError>
    where
        T: Serialize + ?Sized,
    {
        self.send(Method::Patch(json), path, true).await
    }

    pub async fn delete(&self, path: &str) -> Result<serde_json::Value, CliError> {
        self.send::<()>(Method::Delete, path, true).await
    }

    async fn send<T>(
        &self,
        method: Method<&T>,
        path: &str,
        verbose: bool,
    ) -> Result<serde_json::Value, CliError>
    where
        T: Serialize + ?Sized,
    {
        let client = reqwest::Client::new();
        let url = self.host.join(path)?.to_string();

        let attempts = self.attempts.unwrap_or(DEFAULT_ATTEMPTS);
        let max_attempts = attempts.checked_sub(1).ok_or(CliError::BadMath)?;
        let retry_after = self.retry_after.unwrap_or(DEFAULT_RETRY_AFTER);

        for attempt in 0..attempts {
            match self.builder(&client, &method, &url).send().await {
                Ok(res) => {
                    let json = res.json().await?;
                    if verbose {
                        cli_println!("{}", serde_json::to_string_pretty(&json)?);
                    }
                    return Ok(json);
                },
                Err(e) => {
                    cli_println!("Send attempt #{}: {e}", attempt + 1);
                    if attempt != max_attempts {
                        cli_println!("Will retry after {retry_after} second(s).");
                        sleep(Duration::from_secs(retry_after)).await;
                    }
                },
            }
        }

        Err(CliError::Send(attempts))
    }

    fn builder<T>(&self, client: &Client, method: &Method<T>, url: &str) -> RequestBuilder
    where
        T: Serialize,
    {
        let mut builder = match method {
            Method::Get => client.get(url),
            Method::GetQuery(query) => client.get(url).query(&query),
            Method::Post(json) => client.post(url).json(json),
            Method::Put(json) => client.put(url).json(json),
            Method::Patch(json) => client.patch(url).json(json),
            Method::Delete => client.delete(url),
        };
        if let Some(token) = &self.token {
            builder = builder.header("Authorization", format!("Bearer {token}"));
        }
        builder
    }
}

enum Method<T> {
    Get,
    GetQuery(T),
    Post(T),
    Put(T),
    Patch(T),
    Delete,
}
