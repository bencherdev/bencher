#![cfg(feature = "plus")]

use std::{path::Path, sync::Arc};

use bencher_billing::Biller;
use bencher_github_client::{GitHubApp, GitHubClient};
use bencher_google_client::GoogleClient;
use bencher_json::{
    is_bencher_cloud,
    system::config::{
        JsonCloud, JsonGitHub, JsonGitHubApp, JsonGoogle, JsonPlus, JsonRecaptcha, RegistryStorage,
    },
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
    pub github_app: Option<Arc<GitHubApp>>,
    pub google_client: Option<GoogleClient>,
    pub indexer: Option<Indexer>,
    pub stats: StatsSettings,
    pub biller: Option<Biller>,
    pub licensor: Licensor,
    pub recaptcha_client: Option<RecaptchaClient>,
    pub registry_url: Url,
    pub oci_storage: OciStorage,
}

#[derive(Debug, thiserror::Error)]
pub enum PlusError {
    #[error("Failed to handle self-hosted licensing: {0}")]
    LicenseSelfHosted(bencher_license::LicenseError),
    #[error("Failed to handle Bencher Cloud licensing: {0}")]
    LicenseCloud(bencher_license::LicenseError),
    #[error("Failed to set up the GitHub App: {0}")]
    GitHubApp(bencher_github_client::GitHubAppError),
    #[error("Failed to create Google OAuth redirect URL: {0}")]
    RedirectUrl(url::ParseError),
    #[error("Tried to init Bencher Cloud for other Console URL: {0}")]
    BencherCloud(Url),
    #[error("Failed to setup billing: {0}")]
    Billing(bencher_billing::BillingError),
    #[error("{0}")]
    Index(#[from] bencher_schema::context::IndexError),
    #[error("Invalid registry URL: {0}")]
    RegistryUrl(bencher_json::ValidError),
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
            info!(log, "Using local filesystem registry storage");
            return Ok(Self {
                github_client: None,
                github_app: None,
                google_client: None,
                indexer: None,
                stats: StatsSettings::default(),
                biller: None,
                licensor: Licensor::self_hosted().map_err(PlusError::LicenseSelfHosted)?,
                recaptcha_client: None,
                registry_url: default_registry_url(console_url),
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
        let (registry_url, registry_storage, upload_timeout, max_body_size) =
            plus.registry.map_or((None, None, None, None), |registry| {
                (
                    registry.url,
                    registry.storage,
                    Some(registry.upload_timeout),
                    Some(registry.max_body_size),
                )
            });
        let registry_url = registry_url
            .map(|url| url.try_into().map_err(PlusError::RegistryUrl))
            .transpose()?
            .unwrap_or_else(|| default_registry_url(console_url));
        match &registry_storage {
            Some(RegistryStorage::File { .. }) | None => {
                info!(log, "Using local filesystem registry storage");
            },
            Some(RegistryStorage::S3 { .. }) => info!(log, "Using S3 registry storage"),
        }
        let oci_storage = OciStorage::try_from_config(
            log.clone(),
            registry_storage,
            database_path,
            upload_timeout,
            max_body_size,
            None,
        )
        .map_err(PlusError::OciStorage)?;

        let (github_client, github_app) = github(plus.github)?;

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
                github_app,
                google_client,
                indexer: None,
                stats,
                biller: None,
                licensor: Licensor::self_hosted().map_err(PlusError::LicenseSelfHosted)?,
                recaptcha_client: None,
                registry_url,
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
            github_app,
            google_client,
            indexer,
            stats,
            biller,
            licensor,
            recaptcha_client,
            registry_url,
            oci_storage,
        })
    }
}

fn default_registry_url(console_url: &Url) -> Url {
    if is_bencher_cloud(console_url) {
        bencher_json::BENCHER_REGISTRY_URL.clone()
    } else {
        bencher_json::LOCALHOST_BENCHER_REGISTRY_URL.clone()
    }
}

fn github(
    github: Option<JsonGitHub>,
) -> Result<(Option<GitHubClient>, Option<Arc<GitHubApp>>), PlusError> {
    let Some(JsonGitHub {
        client_id,
        client_secret,
        app,
    }) = github
    else {
        return Ok((None, None));
    };
    let github_app = app
        .map(
            |JsonGitHubApp {
                 private_key,
                 webhook_secret,
             }| {
                GitHubApp::new(client_id.clone(), &private_key, webhook_secret).map(Arc::new)
            },
        )
        .transpose()
        .map_err(PlusError::GitHubApp)?;
    Ok((
        Some(GitHubClient::new(client_id, client_secret)),
        github_app,
    ))
}

#[cfg(test)]
mod tests {
    use aws_lc_rs::{
        encoding::{AsDer as _, Pkcs8V1Der},
        rsa::{KeyPair, KeySize},
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use bencher_json::system::config::{JsonGitHub, JsonGitHubApp};
    use rustls::crypto::aws_lc_rs::default_provider;

    use super::{PlusError, github};

    const WEBHOOK_SECRET: &str = "webhook-secret-marker";

    fn config(private_key: &str) -> JsonGitHub {
        JsonGitHub {
            client_id: "Iv1.0123456789abcdef".parse().unwrap(),
            client_secret: "client-secret".parse().unwrap(),
            app: Some(JsonGitHubApp {
                private_key: private_key.parse().unwrap(),
                webhook_secret: WEBHOOK_SECRET.parse().unwrap(),
            }),
        }
    }

    /// A fresh RSA private key, as a PKCS#8 PEM.
    fn private_key() -> String {
        let key_pair = KeyPair::generate(KeySize::Rsa2048).unwrap();
        let der: Pkcs8V1Der<'static> = key_pair.as_der().unwrap();
        format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
            STANDARD.encode(der.as_ref())
        )
    }

    // Kills a config that never builds the app, that hands it any secret but the webhook secret,
    // or that drops a bad key's error.
    #[test]
    fn the_config_builds_the_app() {
        // Another test in this process may have installed it already.
        let _provider = default_provider().install_default();

        let (client, app) = github(Some(config(&private_key()))).unwrap();
        let error = github(Some(config("not-a-key"))).unwrap_err();

        assert!(client.is_some());
        assert_eq!(app.unwrap().webhook_secret().as_ref(), WEBHOOK_SECRET);
        assert!(matches!(error, PlusError::GitHubApp(_)), "{error}");
    }
}
