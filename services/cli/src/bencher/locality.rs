use std::convert::TryFrom;

use crate::{bencher::backend::Backend, cli::CliLocality, BencherError};

#[derive(Debug)]
pub enum Locality {
    Local,
    Backend(Backend),
}

impl TryFrom<CliLocality> for Locality {
    type Error = BencherError;

    fn try_from(locality: CliLocality) -> Result<Self, Self::Error> {
        Ok(if locality.local {
            Self::Local
        } else {
            Self::Backend(locality.backend.try_into()?)
        })
    }
}
