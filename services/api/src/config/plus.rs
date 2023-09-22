#![cfg(feature = "plus")]

use bencher_billing::Biller;
use bencher_json::system::config::JsonPlus;
use bencher_license::Licensor;
use tokio::runtime::Handle;
use url::Url;

use crate::ApiError;

pub struct Plus {
    pub biller: Option<Biller>,
    pub licensor: Licensor,
}

impl Plus {
    pub fn new(endpoint: &Url, plus: Option<JsonPlus>) -> Result<Plus, ApiError> {
        let Some(plus) = plus else {
            return Ok(Self {
                biller: None,
                licensor: Licensor::self_hosted().map_err(ApiError::License)?,
            });
        };

        // The only endpoint that should be using the `plus` section is https://bencher.dev
        if !bencher_plus::is_bencher_dev(endpoint) {
            return Err(ApiError::BencherPlus(endpoint.clone()));
        }

        let biller = Some(tokio::task::block_in_place(move || {
            Handle::current().block_on(async { Biller::new(plus.billing).await })
        })?);
        let licensor = Licensor::bencher_cloud(&plus.license_pem).map_err(ApiError::License)?;

        Ok(Self { biller, licensor })
    }
}
