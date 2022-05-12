use diesel::pg::PgConnection;
use dropshot::ApiDescription;
use std::sync::Mutex;
use util::Registrar;

mod put;

pub struct Api;

impl Registrar<Mutex<PgConnection>> for Api {
    fn register(&self, api: &mut ApiDescription<Mutex<PgConnection>>) -> Result<(), String> {
        api.register(put::api_put_migrate)?;
        api.register(put::api_put_rollback)?;
        Ok(())
    }
}
