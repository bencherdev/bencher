use dropshot::ApiDescription;

pub mod adapters;
pub mod auth;
pub mod ping;
pub mod projects;
pub mod reports;
pub mod testbeds;

use crate::util::{
    registrar::Registrar,
    Context,
};

pub struct Api;

impl Registrar<Context> for Api {
    fn register(&self, api: &mut ApiDescription<Context>) -> Result<(), String> {
        // Ping
        api.register(ping::api_get_ping)?;
        // Auth
        api.register(auth::signup::options)?;
        api.register(auth::signup::post)?;
        api.register(auth::login::options)?;
        api.register(auth::login::post)?;
        // Projects
        api.register(projects::options)?;
        api.register(projects::get_ls)?;
        api.register(projects::post)?;
        api.register(projects::options_params)?;
        api.register(projects::get_one)?;
        // Testbeds
        api.register(testbeds::get_ls_options)?;
        api.register(testbeds::get_ls)?;
        api.register(testbeds::post_options)?;
        api.register(testbeds::post)?;
        api.register(testbeds::get_one_options)?;

        // api.register(testbeds::api_post_testbed)?;
        // api.register(testbeds::api_get_testbed)?;
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
