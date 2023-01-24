use std::path::PathBuf;

pub type DbConnection = diesel::SqliteConnection;

pub struct Database {
    pub path: PathBuf,
    pub connection: DbConnection,
}
