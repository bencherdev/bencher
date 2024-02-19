use clap::Parser;

use crate::{parser::CliBencher, CliError};

pub mod backend;
pub mod sub;

pub use backend::BackendError;
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

impl SubCmd for Bencher {
    async fn exec(&self) -> Result<(), CliError> {
        self.sub.exec().await
    }
}
