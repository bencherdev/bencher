#![cfg(feature = "plus")]

use std::path::Path;

use bencher_billing::Biller;
use bencher_github_client::GitHubClient;
use bencher_google_client::GoogleClient;
use bencher_json::{
    is_bencher_cloud,
    system::config::{JsonCloud, JsonGitHub, JsonGoogle, JsonPlus, JsonRecaptcha},
};
use bencher_license::Licensor;
use bencher_oci_storage::OciStorage;
use bencher_recaptcha::RecaptchaClient;
use bencher_schema::context::{Indexer, StatsSettings};
use slog::{Logger, info};
use tokio::runtime::Handle;
use url::Url;

pub struct Plus {
    pub github_client: Option<GitHubClient>,
    pub google_client: Option<GoogleClient>,
    pub indexer: Option<Indexer>,
    pub stats: StatsSettings,
    pub biller: Option<Biller>,
    pub licensor: Licensor,
    pub recaptcha_client: Option<RecaptchaClient>,
    pub oci_storage: OciStorage,
}

#[derive(Debug, thiserror::Error)]
pub enum PlusError {
    #[error("Failed to handle self-hosted licensing: {0}")]
    LicenseSelfHosted(bencher_license::LicenseError),
    #[error("Failed to handle Bencher Cloud licensing: {0}")]
    LicenseCloud(bencher_license::LicenseError),
    #[error("Failed to create Google OAuth redirect URL: {0}")]
    RedirectUrl(url::ParseError),
    #[error("Tried to init Bencher Cloud for other Console URL: {0}")]
    BencherCloud(Url),
    #[error("Failed to setup billing: {0}")]
    Billing(bencher_billing::BillingError),
    #[error("{0}")]
    Index(#[from] bencher_schema::context::IndexError),
    #[error("Failed to initialize OCI storage: {0}")]
    OciStorage(bencher_oci_storage::OciStorageError),
}

impl Plus {
    #[expect(
        clippy::too_many_lines,
        reason = "registry config adds necessary lines"
    )]
    pub fn new(
        log: &Logger,
        console_url: &Url,
        plus: Option<JsonPlus>,
        database_path: &Path,
    ) -> Result<Self, PlusError> {
        let Some(plus) = plus else {
            // No Plus config, but still provide local OCI storage
            info!(log, "Using local filesystem OCI storage (no S3 configured)");
            return Ok(Self {
                github_client: None,
                google_client: None,
                indexer: None,
                stats: StatsSettings::default(),
                biller: None,
                licensor: Licensor::self_hosted().map_err(PlusError::LicenseSelfHosted)?,
                recaptcha_client: None,
                oci_storage: OciStorage::try_from_config(
                    log.clone(),
                    None,
                    database_path,
                    None,
                    None,
                    None,
                )
                .map_err(PlusError::OciStorage)?,
            });
        };

        // Initialize registry storage - uses S3 if configured, otherwise local filesystem
        let (registry_data_store, upload_timeout, max_body_size) =
            plus.registry.map_or((None, None, None), |registry| {
                (
                    Some(registry.data_store),
                    Some(registry.upload_timeout),
                    Some(registry.max_body_size),
                )
            });
        if registry_data_store.is_none() {
            info!(
                log,
                "Using local filesystem registry storage (no S3 configured)"
            );
        } else {
            info!(log, "Using S3 registry storage");
        }
        let oci_storage = OciStorage::try_from_config(
            log.clone(),
            registry_data_store,
            database_path,
            upload_timeout,
            max_body_size,
            None,
        )
        .map_err(PlusError::OciStorage)?;

        let github_client = plus.github.map(
            |JsonGitHub {
                 client_id,
                 client_secret,
             }| GitHubClient::new(client_id, client_secret),
        );

        let google_client = plus
            .google
            .map(
                |JsonGoogle {
                     client_id,
                     client_secret,
                 }| {
                    console_url
                        .join("/auth/google")
                        .map(|redirect_url| {
                            GoogleClient::new(client_id, client_secret, redirect_url)
                        })
                        .map_err(PlusError::RedirectUrl)
                },
            )
            .transpose()?;

        let stats = plus.stats.map(Into::into).unwrap_or_default();

        let Some(JsonCloud {
            billing,
            license_pem,
            index,
            recaptcha,
            ..
        }) = plus.cloud
        else {
            return Ok(Self {
                github_client,
                google_client,
                indexer: None,
                stats,
                biller: None,
                licensor: Licensor::self_hosted().map_err(PlusError::LicenseSelfHosted)?,
                recaptcha_client: None,
                oci_storage,
            });
        };

        // The only Console URL that should be using the `cloud` section is https://bencher.dev
        if !is_bencher_cloud(console_url) {
            return Err(PlusError::BencherCloud(console_url.clone()));
        }

        let indexer = index.map(TryInto::try_into).transpose()?;

        let biller = Some(
            tokio::task::block_in_place(move || {
                Handle::current().block_on(async { Biller::new(billing).await })
            })
            .map_err(PlusError::Billing)?,
        );
        let licensor = Licensor::bencher_cloud(&license_pem).map_err(PlusError::LicenseCloud)?;

        let recaptcha_client = recaptcha
            .map(|JsonRecaptcha { secret, min_score }| RecaptchaClient::new(secret, min_score));

        Ok(Self {
            github_client,
            google_client,
            indexer,
            stats,
            biller,
            licensor,
            recaptcha_client,
            oci_storage,
        })
    }
}
