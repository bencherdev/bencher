mod messenger;
mod rbac;

pub use messenger::{Email, Messenger};
pub use rbac::Rbac;

pub type Context = tokio::sync::Mutex<ApiContext>;

pub struct ApiContext {
    pub secret_key: String,
    pub rbac: Rbac,
    pub messenger: Messenger,
    pub db_conn: diesel::SqliteConnection,
}
