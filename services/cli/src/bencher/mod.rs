use std::convert::TryFrom;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use clap::Parser;

use crate::{parser::CliBencher, CliError};

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
        Some(to_date_time(timestamp)?)
    } else {
        None
    })
}

fn to_date_time(timestamp: i64) -> Result<DateTime<Utc>, CliError> {
    Utc.timestamp_opt(timestamp, 0)
        .single()
        .ok_or(CliError::DateTime(timestamp))
}
