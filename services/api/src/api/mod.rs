use diesel::sqlite::SqliteConnection;
use dropshot::ApiDescription;
use tokio::sync::Mutex;

pub mod adapters;
pub mod headers;
pub mod ping;
pub mod registrar;
pub mod reports;
pub mod server;

use registrar::Registrar;

pub struct Api;

impl Registrar<Mutex<SqliteConnection>> for Api {
    fn register(&self, api: &mut ApiDescription<Mutex<SqliteConnection>>) -> Result<(), String> {
        api.register(ping::api_get_ping)?;
        api.register(adapters::api_get_adapters)?;
        api.register(adapters::api_get_adapter)?;
        api.register(reports::api_get_reports)?;
        api.register(reports::api_put_report)?;
        Ok(())
    }
}
