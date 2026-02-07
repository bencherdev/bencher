#[cfg(feature = "plus")]
use bencher_billing::Biller;
#[cfg(feature = "plus")]
use bencher_github_client::GitHubClient;
#[cfg(feature = "plus")]
use bencher_google_client::GoogleClient;
#[cfg(feature = "plus")]
use bencher_license::Licensor;
#[cfg(feature = "plus")]
use bencher_oci_storage::OciStorage;
use bencher_token::TokenKey;
#[cfg(feature = "plus")]
use dropshot::HttpError;
use tokio::sync::mpsc::Sender;
use url::Url;

#[cfg(feature = "plus")]
use crate::model::project::QueryProject;

mod database;
#[cfg(feature = "plus")]
mod heartbeat_tasks;
mod indexer;
mod messenger;
#[cfg(feature = "plus")]
mod rate_limiting;
mod rbac;
#[cfg(feature = "plus")]
mod stats;

#[cfg(feature = "plus")]
use bencher_recaptcha::RecaptchaClient;
pub use database::{DataStore, DataStoreError, Database, DbConnection};
#[cfg(feature = "plus")]
pub use heartbeat_tasks::HeartbeatTasks;
#[cfg(feature = "plus")]
pub use indexer::{IndexError, Indexer};
#[cfg(feature = "plus")]
pub use messenger::ServerStatsBody;
pub use messenger::{Body, ButtonBody, Email, Message, Messenger, NewUserBody};
#[cfg(feature = "plus")]
pub use rate_limiting::{HeaderMap, RateLimiting, RateLimitingError};
pub use rbac::{Rbac, RbacError};
#[cfg(feature = "plus")]
pub use stats::StatsSettings;

pub struct ApiContext {
    pub console_url: Url,
    pub request_body_max_bytes: usize,
    pub token_key: TokenKey,
    pub rbac: Rbac,
    pub messenger: Messenger,
    pub database: Database,
    pub restart_tx: Sender<()>,
    #[cfg(feature = "plus")]
    pub rate_limiting: RateLimiting,
    #[cfg(feature = "plus")]
    pub github_client: Option<GitHubClient>,
    #[cfg(feature = "plus")]
    pub google_client: Option<GoogleClient>,
    #[cfg(feature = "plus")]
    pub indexer: Option<Indexer>,
    #[cfg(feature = "plus")]
    pub stats: StatsSettings,
    #[cfg(feature = "plus")]
    pub biller: Option<Biller>,
    #[cfg(feature = "plus")]
    pub licensor: Licensor,
    #[cfg(feature = "plus")]
    pub recaptcha_client: Option<RecaptchaClient>,
    #[cfg(feature = "plus")]
    pub is_bencher_cloud: bool,
    #[cfg(feature = "plus")]
    pub oci_storage: OciStorage,
    #[cfg(feature = "plus")]
    pub heartbeat_timeout: std::time::Duration,
    #[cfg(feature = "plus")]
    pub heartbeat_tasks: HeartbeatTasks,
}

#[macro_export]
macro_rules! public_conn {
    ($context:expr) => {
        &mut *$context.database.get_public_conn().await?
    };
    ($context:expr, $pub_user:expr) => {
        &mut *match $pub_user {
            $crate::model::user::public::PublicUser::Public(_) => {
                $context.database.get_public_conn().await?
            },
            $crate::model::user::public::PublicUser::Auth(_) => {
                $context.database.get_auth_conn().await?
            },
        }
    };
    ($context:expr, $pub_user:expr, |$conn:ident| $multi:expr) => {{
        let $conn = $crate::public_conn!($context, $pub_user);
        $multi
    }};
}

#[macro_export]
macro_rules! auth_conn {
    ($context:expr) => {
        &mut *$context.database.get_auth_conn().await?
    };
    ($context:expr, |$conn:ident| $multi:expr) => {{
        let $conn = $crate::auth_conn!($context);
        $multi
    }};
}

#[macro_export]
macro_rules! write_conn {
    ($context:expr) => {
        &mut *$context.database.connection.lock().await
    };
}

impl ApiContext {
    #[cfg(feature = "plus")]
    pub fn biller(&self) -> Result<&Biller, HttpError> {
        self.biller.as_ref().ok_or_else(|| {
            crate::error::locked_error("Tried to use a Bencher Cloud route when Self-Hosted")
        })
    }

    #[cfg(feature = "plus")]
    pub fn oci_storage(&self) -> &OciStorage {
        &self.oci_storage
    }

    #[cfg(feature = "plus")]
    pub async fn update_index(&self, log: &slog::Logger, query_project: &QueryProject) {
        let Some(indexer) = &self.indexer else {
            return;
        };

        let url = match query_project.perf_url(&self.console_url) {
            Ok(Some(url)) => url,
            Ok(None) => return,
            Err(e) => {
                slog::error!(log, "{e}");
                #[cfg(feature = "sentry")]
                sentry::capture_error(&e);
                return;
            },
        };
        if let Err(e) = indexer.updated(url).await {
            slog::error!(log, "{e}");
            #[cfg(feature = "sentry")]
            sentry::capture_error(&e);
        }
    }

    #[cfg(feature = "plus")]
    pub async fn delete_index(&self, log: &slog::Logger, query_project: &QueryProject) {
        let Some(indexer) = &self.indexer else {
            return;
        };

        let url = match query_project.perf_url(&self.console_url) {
            Ok(Some(url)) => url,
            Ok(None) => return,
            Err(e) => {
                slog::error!(log, "{e}");
                #[cfg(feature = "sentry")]
                sentry::capture_error(&e);
                return;
            },
        };
        if let Err(e) = indexer.deleted(url).await {
            slog::error!(log, "{e}");
            #[cfg(feature = "sentry")]
            sentry::capture_error(&e);
        }
    }
}
