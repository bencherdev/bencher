pub type Context = tokio::sync::Mutex<ApiContext>;

pub struct ApiContext {
    pub secret_key: String,
    pub db_conn: diesel::SqliteConnection,
}
