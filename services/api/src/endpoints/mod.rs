use dropshot::ApiDescription;

mod endpoint;
pub mod method;
pub mod organization;
pub mod project;
pub mod resource;
pub mod system;
pub mod user;

pub use endpoint::Endpoint;
pub use method::Method;
pub use resource::Resource;

use crate::{context::Context, util::registrar::Registrar, ApiError};

pub struct Api;

impl Registrar<Context> for Api {
    fn register(api: &mut ApiDescription<Context>) -> Result<(), ApiError> {
        register(api).map_err(ApiError::Register)
    }
}

fn register(api: &mut ApiDescription<Context>) -> Result<(), String> {
    // Server
    api.register(system::server::ping::options)?;
    api.register(system::server::ping::get)?;
    api.register(system::server::version::options)?;
    api.register(system::server::version::get)?;
    api.register(system::server::restart::options)?;
    api.register(system::server::restart::post)?;
    api.register(system::server::config::options)?;
    api.register(system::server::config::put)?;
    api.register(system::server::config::get_one)?;
    // Auth
    api.register(system::auth::signup::options)?;
    api.register(system::auth::signup::post)?;
    api.register(system::auth::login::options)?;
    api.register(system::auth::login::post)?;
    api.register(system::auth::confirm::options)?;
    api.register(system::auth::confirm::post)?;

    // Organizations
    api.register(organization::organizations::dir_options)?;
    api.register(organization::organizations::get_ls)?;
    api.register(organization::organizations::post)?;
    api.register(organization::organizations::one_options)?;
    api.register(organization::organizations::get_one)?;
    // Organization Permission
    api.register(organization::allowed::options)?;
    api.register(organization::allowed::get)?;
    // Organization Members
    api.register(organization::members::dir_options)?;
    api.register(organization::members::get_ls)?;
    api.register(organization::members::post)?;
    api.register(organization::members::one_options)?;
    api.register(organization::members::get_one)?;
    api.register(organization::members::patch)?;
    // Organization Projects
    api.register(organization::projects::dir_options)?;
    api.register(organization::projects::get_ls)?;
    api.register(organization::projects::post)?;
    api.register(organization::projects::one_options)?;
    api.register(organization::projects::get_one)?;

    // Projects
    // All of a projects's GET APIs and its Perf POST API
    // are public if the project is public
    api.register(project::projects::dir_options)?;
    api.register(project::projects::get_ls)?;
    api.register(project::projects::one_options)?;
    api.register(project::projects::get_one)?;
    // Perf
    api.register(project::perf::options)?;
    api.register(project::perf::post)?;
    // Reports
    api.register(project::reports::dir_options)?;
    api.register(project::reports::get_ls)?;
    api.register(project::reports::post)?;
    api.register(project::reports::one_options)?;
    api.register(project::reports::get_one)?;
    // Results
    api.register(project::results::one_options)?;
    api.register(project::results::get_one)?;
    // Branches
    api.register(project::branches::dir_options)?;
    api.register(project::branches::get_ls)?;
    api.register(project::branches::post)?;
    api.register(project::branches::one_options)?;
    api.register(project::branches::get_one)?;
    // Testbeds
    api.register(project::testbeds::dir_options)?;
    api.register(project::testbeds::get_ls)?;
    api.register(project::testbeds::post)?;
    api.register(project::testbeds::one_options)?;
    api.register(project::testbeds::get_one)?;
    // Benchmarks
    api.register(project::benchmarks::dir_options)?;
    api.register(project::benchmarks::get_ls)?;
    api.register(project::benchmarks::one_options)?;
    api.register(project::benchmarks::get_one)?;
    // Metric Kinds
    api.register(project::metric_kinds::dir_options)?;
    api.register(project::metric_kinds::get_ls)?;
    api.register(project::metric_kinds::post)?;
    api.register(project::metric_kinds::one_options)?;
    api.register(project::metric_kinds::get_one)?;
    // Thresholds
    api.register(project::thresholds::dir_options)?;
    api.register(project::thresholds::get_ls)?;
    api.register(project::thresholds::post)?;
    api.register(project::thresholds::one_options)?;
    api.register(project::thresholds::get_one)?;
    // Alerts
    api.register(project::alerts::dir_options)?;
    api.register(project::alerts::get_ls)?;
    api.register(project::alerts::one_options)?;
    api.register(project::alerts::get_one)?;

    // Users
    api.register(user::users::one_options)?;
    api.register(user::users::get_one)?;
    // Tokens
    api.register(user::tokens::dir_options)?;
    api.register(user::tokens::get_ls)?;
    api.register(user::tokens::post)?;
    api.register(user::tokens::one_options)?;
    api.register(user::tokens::get_one)?;

    Ok(())
}
