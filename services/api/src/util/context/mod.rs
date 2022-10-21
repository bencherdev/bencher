use url::Url;

mod messenger;
mod rbac;
mod secret_key;

pub use messenger::{Body, ButtonBody, Email, Message, Messenger};
pub use rbac::Rbac;
pub use secret_key::SecretKey;

pub type Context = tokio::sync::Mutex<ApiContext>;

pub struct ApiContext {
    pub endpoint: Url,
    pub secret_key: SecretKey,
    pub rbac: Rbac,
    pub messenger: Messenger,
    pub database: diesel::SqliteConnection,
}
