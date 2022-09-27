mod email;
mod rbac;

pub use email::Email;
pub use rbac::Rbac;

pub type Context = tokio::sync::Mutex<ApiContext>;

pub struct ApiContext {
    pub secret_key: String,
    pub rbac: Rbac,
    pub email: Option<Email>,
    pub db_conn: diesel::SqliteConnection,
}
