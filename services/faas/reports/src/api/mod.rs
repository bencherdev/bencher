use std::sync::Mutex;

use diesel::pg::PgConnection;
use dropshot::ApiDescription;
use util::Registrar;

pub mod get;
pub mod put;

pub struct Api;

impl Registrar<Mutex<PgConnection>> for Api {
    fn register(&self, api: &mut ApiDescription<Mutex<PgConnection>>) -> Result<(), String> {
        api.register(get::api_get_metrics)?;
        api.register(put::api_put_reports)?;
        Ok(())
    }
}
