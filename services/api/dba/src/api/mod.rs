use dropshot::ApiDescription;

use util::Registrar;

mod put;

pub struct Api;

impl Registrar<()> for Api {
    fn register(&self, api: &mut ApiDescription<()>) -> Result<(), String> {
        api.register(put::api_put_migrate)?;
        Ok(())
    }
}
