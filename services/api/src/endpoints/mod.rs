use diesel::sqlite::SqliteConnection;
use dropshot::ApiDescription;
use tokio::sync::Mutex;

pub mod adapters;
pub mod ping;
pub mod reports;
pub mod testbeds;

use crate::util::registrar::Registrar;

pub struct Api;

impl Registrar<Mutex<SqliteConnection>> for Api {
    fn register(&self, api: &mut ApiDescription<Mutex<SqliteConnection>>) -> Result<(), String> {
        api.register(ping::api_get_ping)?;
        // Testbeds
        api.register(testbeds::api_get_testbeds)?;
        api.register(testbeds::api_get_testbed)?;
        // api.register(testbeds::api_post_testbed)?;
        // Adapters
        api.register(adapters::api_get_adapters)?;
        api.register(adapters::api_get_adapter)?;
        // Reports
        api.register(reports::api_get_reports)?;
        api.register(reports::api_get_report)?;
        api.register(reports::api_post_report)?;
        Ok(())
    }
}
