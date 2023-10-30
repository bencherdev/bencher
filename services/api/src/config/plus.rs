#![cfg(feature = "plus")]

use bencher_billing::Biller;
use bencher_json::system::config::JsonPlus;
use bencher_license::Licensor;
use bencher_plus::BENCHER_DEV;
use tokio::runtime::Handle;
use url::Url;

pub struct Plus {
    pub biller: Option<Biller>,
    pub licensor: Licensor,
}

#[derive(Debug, thiserror::Error)]
pub enum PlusError {
    #[error("Failed to handle self-hosted licensing: {0}")]
    LicenseSelfHosted(bencher_license::LicenseError),
    #[error("Failed to handle Bencher Cloud licensing: {0}")]
    LicenseCloud(bencher_license::LicenseError),
    #[error(
        "Tried to init Bencher Cloud for endpoint ({0}) other than {url}",
        url = BENCHER_DEV
    )]
    BencherCloud(Url),
    #[error("Failed to setup billing: {0}")]
    Billing(bencher_billing::BillingError),
}

impl Plus {
    pub fn new(endpoint: &Url, plus: Option<JsonPlus>) -> Result<Self, PlusError> {
        match Self::new_inner(endpoint, plus)? {
            Some(plus) => Ok(plus),
            None => Ok(Self {
                biller: None,
                licensor: Licensor::self_hosted().map_err(PlusError::LicenseSelfHosted)?,
            }),
        }
    }

    pub fn new_inner(endpoint: &Url, plus: Option<JsonPlus>) -> Result<Option<Self>, PlusError> {
        let Some(plus) = plus else {
            return Ok(None);
        };

        let Some(cloud) = plus.cloud else {
            return Ok(None);
        };

        // The only endpoint that should be using the `cloud` section is https://bencher.dev
        if !bencher_plus::is_bencher_dev(endpoint) {
            return Err(PlusError::BencherCloud(endpoint.clone()));
        }

        let biller = Some(
            tokio::task::block_in_place(move || {
                Handle::current().block_on(async { Biller::new(cloud.billing).await })
            })
            .map_err(PlusError::Billing)?,
        );
        let licensor =
            Licensor::bencher_cloud(&cloud.license_pem).map_err(PlusError::LicenseCloud)?;

        Ok(Some(Self { biller, licensor }))
    }
}
