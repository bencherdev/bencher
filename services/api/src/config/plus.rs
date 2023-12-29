#![cfg(feature = "plus")]

use bencher_billing::Biller;
use bencher_github::GitHub;
use bencher_json::{
    is_bencher_cloud,
    system::config::{JsonPlus, JsonStats},
};
use bencher_license::Licensor;
use chrono::NaiveTime;
use once_cell::sync::Lazy;
use tokio::runtime::Handle;
use url::Url;

// Run at 03:07:22 UTC by default (offset 11,242 seconds)
#[allow(clippy::expect_used)]
static DEFAULT_STATS_OFFSET: Lazy<NaiveTime> =
    Lazy::new(|| NaiveTime::from_hms_opt(3, 7, 22).expect("Invalid default stats offset"));
// Default stats to enabled
const DEFAULT_STATS_ENABLED: bool = true;

pub struct Plus {
    pub github: Option<GitHub>,
    pub stats: StatsSettings,
    pub biller: Option<Biller>,
    pub licensor: Licensor,
}

#[derive(Debug, thiserror::Error)]
pub enum PlusError {
    #[error("Failed to handle self-hosted licensing: {0}")]
    LicenseSelfHosted(bencher_license::LicenseError),
    #[error("Failed to handle Bencher Cloud licensing: {0}")]
    LicenseCloud(bencher_license::LicenseError),
    #[error("Tried to init Bencher Cloud for other endpoint: {0}")]
    BencherCloud(Url),
    #[error("Failed to setup billing: {0}")]
    Billing(bencher_billing::BillingError),
}

impl Plus {
    pub fn new(endpoint: &Url, plus: Option<JsonPlus>) -> Result<Self, PlusError> {
        let Some(plus) = plus else {
            return Ok(Self {
                github: None,
                stats: StatsSettings::default(),
                biller: None,
                licensor: Licensor::self_hosted().map_err(PlusError::LicenseSelfHosted)?,
            });
        };

        let github = plus
            .github
            .map(|github| GitHub::new(github.client_id, github.client_secret));

        let stats = plus.stats.map(Into::into).unwrap_or_default();

        let Some(cloud) = plus.cloud else {
            return Ok(Self {
                github,
                stats,
                biller: None,
                licensor: Licensor::self_hosted().map_err(PlusError::LicenseSelfHosted)?,
            });
        };

        // The only endpoint that should be using the `cloud` section is https://bencher.dev
        if !is_bencher_cloud(endpoint) {
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

        Ok(Self {
            github,
            stats,
            biller,
            licensor,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StatsSettings {
    pub offset: NaiveTime,
    pub enabled: bool,
}

impl Default for StatsSettings {
    fn default() -> Self {
        Self {
            offset: *DEFAULT_STATS_OFFSET,
            enabled: DEFAULT_STATS_ENABLED,
        }
    }
}

impl From<JsonStats> for StatsSettings {
    fn from(json: JsonStats) -> Self {
        let JsonStats { offset, enabled } = json;
        let offset = offset
            .and_then(|offset| NaiveTime::from_num_seconds_from_midnight_opt(offset, 0))
            .unwrap_or(*DEFAULT_STATS_OFFSET);
        let enabled = enabled.unwrap_or(DEFAULT_STATS_ENABLED);
        Self { offset, enabled }
    }
}
