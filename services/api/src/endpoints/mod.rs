use dropshot::ApiDescription;

pub mod adapters;
pub mod auth;
pub mod benchmarks;
pub mod branches;
pub mod perf;
pub mod ping;
pub mod projects;
pub mod reports;
pub mod testbeds;
pub mod thresholds;

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
        api.register(projects::dir_options)?;
        api.register(projects::get_ls)?;
        api.register(projects::post)?;
        api.register(projects::one_options)?;
        api.register(projects::get_one)?;
        // Perf
        api.register(perf::options)?;
        api.register(perf::post)?;
        // Reports
        api.register(reports::dir_options)?;
        api.register(reports::get_ls)?;
        api.register(reports::post_options)?;
        api.register(reports::post)?;
        api.register(reports::one_options)?;
        api.register(reports::get_one)?;
        // Branches
        api.register(branches::dir_options)?;
        api.register(branches::get_ls)?;
        api.register(branches::post_options)?;
        api.register(branches::post)?;
        api.register(branches::one_options)?;
        api.register(branches::get_one)?;
        // Testbeds
        api.register(testbeds::dir_options)?;
        api.register(testbeds::get_ls)?;
        api.register(testbeds::post_options)?;
        api.register(testbeds::post)?;
        api.register(testbeds::one_options)?;
        api.register(testbeds::get_one)?;
        // Adapters
        api.register(adapters::dir_options)?;
        api.register(adapters::get_ls)?;
        api.register(adapters::one_options)?;
        api.register(adapters::get_one)?;
        // Benchmarks
        api.register(benchmarks::dir_options)?;
        api.register(benchmarks::get_ls)?;
        api.register(benchmarks::one_options)?;
        api.register(benchmarks::get_one)?;
        // Thresholds
        api.register(thresholds::dir_options)?;
        api.register(thresholds::get_ls)?;
        api.register(thresholds::post_options)?;
        api.register(thresholds::post)?;
        api.register(thresholds::one_options)?;
        api.register(thresholds::get_one)?;

        Ok(())
    }
}
