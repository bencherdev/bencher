use dropshot::ApiDescription;

pub mod adapters;
pub mod auth;
pub mod branches;
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
        // Reports
        api.register(reports::get_ls_options)?;
        api.register(reports::get_ls)?;
        api.register(reports::post_options)?;
        api.register(reports::post)?;
        // api.register(reports::get_one_options)?;
        // api.register(reports::get_one)?;
        // Branches
        api.register(branches::get_ls_options)?;
        api.register(branches::get_ls)?;
        api.register(branches::post_options)?;
        api.register(branches::post)?;
        api.register(branches::get_one_options)?;
        api.register(branches::get_one)?;
        // Testbeds
        api.register(testbeds::get_ls_options)?;
        api.register(testbeds::get_ls)?;
        api.register(testbeds::post_options)?;
        api.register(testbeds::post)?;
        api.register(testbeds::get_one_options)?;
        api.register(testbeds::get_one)?;

        // Adapters
        api.register(adapters::get_ls_options)?;
        api.register(adapters::get_ls)?;
        api.register(adapters::get_one_options)?;
        api.register(adapters::get_one)?;

        Ok(())
    }
}
