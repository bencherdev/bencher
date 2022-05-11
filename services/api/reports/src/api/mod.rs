use dropshot::ApiDescription;

use util::Registrar;

mod get;
mod put;

pub struct Api;

impl Registrar<()> for Api {
    fn register(&self, api: &mut ApiDescription<()>) -> Result<(), String> {
        api.register(get::api_get_reports)?;
        api.register(put::api_put_reports)?;
        Ok(())
    }
}
