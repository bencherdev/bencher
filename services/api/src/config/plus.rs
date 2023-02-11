#![cfg(feature = "plus")]

use bencher_billing::Biller;
use bencher_json::system::config::JsonPlus;
use bencher_license::Licensor;
use url::Url;

use crate::ApiError;

pub struct Plus {
    pub licensor: Licensor,
    pub biller: Biller,
}

impl Plus {
    pub fn new(endpoint: &Url, plus: Option<JsonPlus>) -> Result<Plus, ApiError> {
        let Some(plus) = plus else {
            return Ok(Self {
                licensor: Licensor::self_hosted().map_err(ApiError::License)?,
                biller: Biller::self_hosted()
            });
        };

        // The only endpoint that should be using the `plus` section is https://bencher.dev
        if !bencher_plus::is_bencher_dev(endpoint) {
            return Err(ApiError::BencherPlus(endpoint.clone()));
        }

        let licensor = Licensor::bencher_cloud(plus.license_pem).map_err(ApiError::License)?;

        let biller = Biller::bencher_cloud()?;

        Ok(Self { licensor, biller })
    }
}
