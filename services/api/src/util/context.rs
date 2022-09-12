pub type Context = tokio::sync::Mutex<ApiContext>;

pub struct ApiContext {
    pub key: String,
    pub db: diesel::SqliteConnection,
}
