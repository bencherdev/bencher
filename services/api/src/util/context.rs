use oso::Oso;

pub type Context = tokio::sync::Mutex<ApiContext>;

pub struct ApiContext {
    pub secret_key: String,
    pub oso_rbac: Oso,
    pub db_conn: diesel::SqliteConnection,
}
