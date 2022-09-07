pub type Context = tokio::sync::Mutex<ApiContext>;

pub struct ApiContext {
    pub db: diesel::SqliteConnection,
    pub key: String,
}
