use dropshot::ApiDescription;

pub mod admin;
pub mod auth;
mod endpoint;
pub mod method;
pub mod orgs;
pub mod ping;
pub mod resource;
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
    // Ping
    api.register(ping::get)?;

    // Admin
    api.register(admin::restart::post_options)?;
    api.register(admin::restart::post)?;
    api.register(admin::config::options)?;
    api.register(admin::config::post)?;
    api.register(admin::config::get_one)?;

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
    // Members
    api.register(members::dir_options)?;
    api.register(members::get_ls)?;
    // Invite
    api.register(invites::options)?;
    api.register(invites::post)?;
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
