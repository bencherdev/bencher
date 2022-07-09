use diesel::sqlite::SqliteConnection;
use dropshot::ApiDescription;
use tokio::sync::Mutex;

pub mod endpoints;
pub mod headers;
pub mod registrar;
pub mod server;

use registrar::Registrar;

pub struct Api;

impl Registrar<Mutex<SqliteConnection>> for Api {
    fn register(&self, api: &mut ApiDescription<Mutex<SqliteConnection>>) -> Result<(), String> {
        api.register(endpoints::ping::api_get_ping)?;
        api.register(endpoints::adapters::api_get_adapters)?;
        api.register(endpoints::adapters::api_get_adapter)?;
        api.register(endpoints::reports::api_get_reports)?;
        api.register(endpoints::reports::api_get_report)?;
        api.register(endpoints::reports::api_post_report)?;
        Ok(())
    }
}
