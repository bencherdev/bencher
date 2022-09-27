mod rbac;

pub use rbac::Rbac;

pub type Context = tokio::sync::Mutex<ApiContext>;

pub struct ApiContext {
    pub secret_key: String,
    pub rbac: Rbac,
    pub db_conn: diesel::SqliteConnection,
    // pub email: Option<Email>,
}
