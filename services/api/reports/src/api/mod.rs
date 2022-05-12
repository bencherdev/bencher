use std::sync::Mutex;

use diesel::pg::PgConnection;
use dropshot::ApiDescription;
use util::Registrar;

mod get;
mod put;

pub struct Api;

impl Registrar<Mutex<PgConnection>> for Api {
    fn register(&self, api: &mut ApiDescription<Mutex<PgConnection>>) -> Result<(), String> {
        api.register(get::api_get_reports)?;
        api.register(put::api_put_reports)?;
        Ok(())
    }
}
