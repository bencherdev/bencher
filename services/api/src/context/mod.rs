#[cfg(feature = "plus")]
use bencher_billing::Biller;
#[cfg(feature = "plus")]
use bencher_license::Licensor;
use bencher_selfie::Selfie;
use tokio::sync::mpsc::Sender;
use url::Url;

mod database;
mod messenger;
mod rbac;
mod secret_key;

pub use database::{Database, DbConnection};
pub use messenger::{Body, ButtonBody, Email, Message, Messenger};
pub use rbac::Rbac;
pub use secret_key::SecretKey;

pub type Context = tokio::sync::Mutex<ApiContext>;

pub struct ApiContext {
    pub endpoint: Url,
    pub secret_key: SecretKey,
    pub rbac: Rbac,
    pub messenger: Messenger,
    pub database: Database,
    pub restart_tx: Sender<()>,
    pub selfie: Selfie,
    #[cfg(feature = "plus")]
    pub biller: Option<Biller>,
    #[cfg(feature = "plus")]
    pub licensor: Licensor,
}
