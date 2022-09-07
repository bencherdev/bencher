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
        if locality.local == true {
            Ok(Self::Local)
        } else {
            Ok(Self::Backend(locality.backend.try_into()?))
        }
    }
}
