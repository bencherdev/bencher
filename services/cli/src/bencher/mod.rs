use std::convert::TryFrom;

use async_trait::async_trait;
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
