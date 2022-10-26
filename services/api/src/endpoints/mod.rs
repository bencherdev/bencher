use dropshot::ApiDescription;

pub mod auth;
mod endpoint;
pub mod method;
pub mod orgs;
pub mod resource;
pub mod server;
pub mod users;

pub use endpoint::Endpoint;
pub use method::Method;
use orgs::*;
pub use resource::Resource;

use crate::{
    util::{registrar::Registrar, Context},
    ApiError,
};

pub struct Api;

impl Registrar<Context> for Api {
    fn register(api: &mut ApiDescription<Context>) -> Result<(), ApiError> {
        register(api).map_err(ApiError::Register)
    }
}

fn register(api: &mut ApiDescription<Context>) -> Result<(), String> {
    // Server
    api.register(server::ping::options)?;
    api.register(server::ping::get)?;
    api.register(server::restart::options)?;
    api.register(server::restart::post)?;
    api.register(server::config::options)?;
    api.register(server::config::post)?;
    api.register(server::config::get_one)?;

    // Auth
    api.register(auth::signup::options)?;
    api.register(auth::signup::post)?;
    api.register(auth::login::options)?;
    api.register(auth::login::post)?;
    api.register(auth::confirm::options)?;
    api.register(auth::confirm::post)?;
    // Organizations
    api.register(organizations::dir_options)?;
    api.register(organizations::get_ls)?;
    api.register(organizations::post)?;
    api.register(organizations::one_options)?;
    api.register(organizations::get_one)?;
    api.register(organizations::allowed_options)?;
    api.register(organizations::get_allowed)?;
    // Members
    api.register(members::dir_options)?;
    api.register(members::get_ls)?;
    api.register(members::post_options)?;
    api.register(members::post)?;
    api.register(members::one_options)?;
    api.register(members::get_one)?;
    api.register(members::patch)?;
    // Projects
    api.register(projects::dir_options)?;
    api.register(projects::get_ls)?;
    api.register(projects::post)?;
    api.register(projects::one_options)?;
    api.register(projects::get_one)?;
    api.register(projects::one_project_options)?;
    api.register(projects::get_one_project)?;
    // Perf
    api.register(perf::options)?;
    api.register(perf::put)?;
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
    // Alerts
    api.register(alerts::dir_options)?;
    api.register(alerts::get_ls)?;
    api.register(alerts::one_options)?;
    api.register(alerts::get_one)?;

    // Tokens
    api.register(users::tokens::dir_options)?;
    api.register(users::tokens::get_ls)?;
    api.register(users::tokens::post_options)?;
    api.register(users::tokens::post)?;
    api.register(users::tokens::one_options)?;
    api.register(users::tokens::get_one)?;

    Ok(())
}
