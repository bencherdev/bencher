use std::convert::TryFrom;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use clap::Parser;

use crate::{cli::CliBencher, CliError};

pub mod backend;
pub mod sub;

use sub::{Sub, SubCmd};

#[derive(Debug)]
pub struct Bencher {
    sub: Sub,
}

impl TryFrom<CliBencher> for Bencher {
    type Error = CliError;

    fn try_from(bencher: CliBencher) -> Result<Self, Self::Error> {
        Ok(Self {
            sub: bencher.sub.try_into()?,
        })
    }
}

impl Bencher {
    pub fn new() -> Result<Self, CliError> {
        CliBencher::parse().try_into()
    }
}

#[async_trait]
impl SubCmd for Bencher {
    async fn exec(&self) -> Result<(), CliError> {
        self.sub.exec().await
    }
}

fn map_timestamp(timestamp: Option<i64>) -> Result<Option<DateTime<Utc>>, CliError> {
    Ok(if let Some(timestamp) = timestamp {
        Some(
            Utc.timestamp_opt(timestamp, 0)
                .single()
                .ok_or(CliError::DateTime(timestamp))?,
        )
    } else {
        None
    })
}

fn from_response<T>(json_value: serde_json::Value) -> Result<T, CliError>
where
    T: serde::de::DeserializeOwned,
{
    match serde_json::from_value(json_value) {
        Ok(value) => Ok(value),
        Err(_) => Err(CliError::RequestFailed),
    }
}
