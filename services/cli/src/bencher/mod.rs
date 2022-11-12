use std::convert::TryFrom;

use clap::Parser;

use crate::{cli::CliBencher, CliError};

pub mod backend;
pub mod locality;
pub mod sub;
pub mod wide;

use sub::{Sub, SubCmd};
use wide::Wide;

#[derive(Debug)]
pub struct Bencher {
    wide: Wide,
    sub: Sub,
}

impl TryFrom<CliBencher> for Bencher {
    type Error = CliError;

    fn try_from(bencher: CliBencher) -> Result<Self, Self::Error> {
        Ok(Self {
            wide: Wide::from(bencher.wide),
            sub: bencher.sub.try_into()?,
        })
    }
}

impl Bencher {
    pub fn new() -> Result<Self, CliError> {
        CliBencher::parse().try_into()
    }

    pub async fn exec(&self) -> Result<(), CliError> {
        self.sub.exec(&self.wide).await
    }

    // TODO actually implement this ping / check auth endpoint
    pub async fn ping(&self) -> Result<(), CliError> {
        Ok(())
    }
}
