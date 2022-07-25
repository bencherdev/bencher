use std::convert::TryFrom;

use clap::Parser;

use crate::{
    cmd::CliBencher,
    BencherError,
};

pub mod adapter;
pub mod backend;
pub mod benchmark;
pub mod locality;
pub mod sub;
pub mod wide;

use sub::{
    map_sub,
    Sub,
    SubCmd,
};
use wide::Wide;

#[derive(Debug)]
pub struct Bencher {
    wide: Wide,
    sub:  Option<Sub>,
}

impl TryFrom<CliBencher> for Bencher {
    type Error = BencherError;

    fn try_from(bencher: CliBencher) -> Result<Self, Self::Error> {
        Ok(Self {
            wide: Wide::from(bencher.wide),
            sub:  map_sub(bencher.sub)?,
        })
    }
}

impl Bencher {
    pub fn new() -> Result<Self, BencherError> {
        let args = CliBencher::parse();
        Self::try_from(args)
    }

    pub async fn exec(&self) -> Result<(), BencherError> {
        if let Some(sub) = &self.sub {
            sub.exec(&self.wide).await
        } else {
            self.ping().await
        }
    }

    // TODO actually implement this ping / check auth endpoint
    pub async fn ping(&self) -> Result<(), BencherError> {
        Ok(())
    }
}
