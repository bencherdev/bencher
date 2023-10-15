#[cfg(feature = "plus")]
use bencher_billing::Biller;
#[cfg(feature = "plus")]
use bencher_license::Licensor;
use tokio::sync::mpsc::Sender;
use url::Url;

mod database;
mod messenger;
mod rbac;
mod secret_key;

pub use database::{DataStoreError, Database, DbConnection};
pub use messenger::{Body, ButtonBody, Email, Message, Messenger, NewUserBody};
pub use rbac::{Rbac, RbacError};
pub use secret_key::{JwtError, SecretKey};

pub struct ApiContext {
    pub endpoint: Url,
    pub secret_key: SecretKey,
    pub rbac: Rbac,
    pub messenger: Messenger,
    pub database: Database,
    pub restart_tx: Sender<()>,
    #[cfg(feature = "plus")]
    pub biller: Option<Biller>,
    #[cfg(feature = "plus")]
    pub licensor: Licensor,
}

impl ApiContext {
    pub async fn conn(&self) -> tokio::sync::MutexGuard<DbConnection> {
        self.database.connection.lock().await
    }
}
