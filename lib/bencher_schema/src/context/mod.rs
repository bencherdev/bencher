#[cfg(feature = "plus")]
use bencher_billing::Biller;
#[cfg(feature = "plus")]
use bencher_github_client::GitHubClient;
#[cfg(feature = "plus")]
use bencher_google_client::GoogleClient;
#[cfg(feature = "plus")]
use bencher_license::Licensor;
use bencher_token::TokenKey;
use tokio::sync::mpsc::Sender;
use url::Url;

#[cfg(feature = "plus")]
use crate::model::project::QueryProject;

mod database;
mod indexer;
mod messenger;
#[cfg(feature = "plus")]
mod rate_limiting;
mod rbac;
#[cfg(feature = "plus")]
mod stats;

pub use database::{DataStore, DataStoreError, Database, DbConnection};
#[cfg(feature = "plus")]
pub use indexer::{IndexError, Indexer};
#[cfg(feature = "plus")]
pub use messenger::ServerStatsBody;
pub use messenger::{Body, ButtonBody, Email, Message, Messenger, NewUserBody};
#[cfg(feature = "plus")]
pub use rate_limiting::{RateLimiting, RateLimitingError};
pub use rbac::{Rbac, RbacError};
#[cfg(feature = "plus")]
pub use stats::StatsSettings;

pub struct ApiContext {
    pub console_url: Url,
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
    pub is_bencher_cloud: bool,
}

#[macro_export]
/// Warning: Do not call `conn_lock!` multiple times in the same line, as it will deadlock.
/// Use the `|conn|` syntax to reuse the same connection multiple times in the same line.
macro_rules! conn_lock {
    ($context:expr) => {
        $crate::connection_lock!($context.database.connection)
    };
    ($context:expr, |$conn:ident| $multi:expr) => {{
        let $conn = $crate::conn_lock!($context);
        $multi
    }};
}

#[macro_export]
/// Warning: Do not call `connection_lock!` multiple times in the same line, as it will deadlock.
/// Use the `|conn|` syntax to reuse the same connection multiple times in the same line.
macro_rules! connection_lock {
    ($connection:expr) => {
        &mut *$connection.lock().await
    };
    ($connection:expr, |$conn:ident| $multi:expr) => {{
        let $conn = $crate::connection_lock!($connection);
        $multi
    }};
}

impl ApiContext {
    #[cfg(feature = "plus")]
    pub fn biller(&self) -> Result<&Biller, dropshot::HttpError> {
        self.biller.as_ref().ok_or_else(|| {
            crate::error::locked_error("Tried to use a Bencher Cloud route when Self-Hosted")
        })
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
