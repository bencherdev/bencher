use dropshot::ApiDescription;

pub mod auth;
pub mod orgs;
pub mod ping;
pub mod users;

use orgs::*;

use crate::{
    util::{registrar::Registrar, Context},
    ApiError,
};

pub struct Api;

impl Registrar<Context> for Api {
    fn register(api: &mut ApiDescription<Context>) -> Result<(), ApiError> {
        // Ping
        api.register(ping::api_get_ping)
            .map_err(ApiError::Endpoint)?;
        // Auth
        api.register(auth::signup::options)
            .map_err(ApiError::Endpoint)?;
        api.register(auth::signup::post)
            .map_err(ApiError::Endpoint)?;
        api.register(auth::login::options)
            .map_err(ApiError::Endpoint)?;
        api.register(auth::login::post)
            .map_err(ApiError::Endpoint)?;
        api.register(auth::confirm::options)
            .map_err(ApiError::Endpoint)?;
        api.register(auth::confirm::post)
            .map_err(ApiError::Endpoint)?;
        api.register(auth::invite::options)
            .map_err(ApiError::Endpoint)?;
        api.register(auth::invite::post)
            .map_err(ApiError::Endpoint)?;
        // Projects
        api.register(projects::dir_options)
            .map_err(ApiError::Endpoint)?;
        api.register(projects::get_ls).map_err(ApiError::Endpoint)?;
        api.register(projects::post).map_err(ApiError::Endpoint)?;
        api.register(projects::one_options)
            .map_err(ApiError::Endpoint)?;
        api.register(projects::get_one)
            .map_err(ApiError::Endpoint)?;
        // Perf
        api.register(perf::options).map_err(ApiError::Endpoint)?;
        api.register(perf::post).map_err(ApiError::Endpoint)?;
        // Reports
        api.register(reports::dir_options)
            .map_err(ApiError::Endpoint)?;
        api.register(reports::get_ls).map_err(ApiError::Endpoint)?;
        api.register(reports::post_options)
            .map_err(ApiError::Endpoint)?;
        api.register(reports::post).map_err(ApiError::Endpoint)?;
        api.register(reports::one_options)
            .map_err(ApiError::Endpoint)?;
        api.register(reports::get_one).map_err(ApiError::Endpoint)?;
        // Branches
        api.register(branches::dir_options)
            .map_err(ApiError::Endpoint)?;
        api.register(branches::get_ls).map_err(ApiError::Endpoint)?;
        api.register(branches::post_options)
            .map_err(ApiError::Endpoint)?;
        api.register(branches::post).map_err(ApiError::Endpoint)?;
        api.register(branches::one_options)
            .map_err(ApiError::Endpoint)?;
        api.register(branches::get_one)
            .map_err(ApiError::Endpoint)?;
        // Testbeds
        api.register(testbeds::dir_options)
            .map_err(ApiError::Endpoint)?;
        api.register(testbeds::get_ls).map_err(ApiError::Endpoint)?;
        api.register(testbeds::post_options)
            .map_err(ApiError::Endpoint)?;
        api.register(testbeds::post).map_err(ApiError::Endpoint)?;
        api.register(testbeds::one_options)
            .map_err(ApiError::Endpoint)?;
        api.register(testbeds::get_one)
            .map_err(ApiError::Endpoint)?;
        // Benchmarks
        api.register(benchmarks::dir_options)
            .map_err(ApiError::Endpoint)?;
        api.register(benchmarks::get_ls)
            .map_err(ApiError::Endpoint)?;
        api.register(benchmarks::one_options)
            .map_err(ApiError::Endpoint)?;
        api.register(benchmarks::get_one)
            .map_err(ApiError::Endpoint)?;
        // Thresholds
        api.register(thresholds::dir_options)
            .map_err(ApiError::Endpoint)?;
        api.register(thresholds::get_ls)
            .map_err(ApiError::Endpoint)?;
        api.register(thresholds::post_options)
            .map_err(ApiError::Endpoint)?;
        api.register(thresholds::post).map_err(ApiError::Endpoint)?;
        api.register(thresholds::one_options)
            .map_err(ApiError::Endpoint)?;
        api.register(thresholds::get_one)
            .map_err(ApiError::Endpoint)?;

        // Tokens
        api.register(users::tokens::dir_options)
            .map_err(ApiError::Endpoint)?;
        api.register(users::tokens::get_ls)
            .map_err(ApiError::Endpoint)?;
        api.register(users::tokens::post_options)
            .map_err(ApiError::Endpoint)?;
        api.register(users::tokens::post)
            .map_err(ApiError::Endpoint)?;
        api.register(users::tokens::one_options)
            .map_err(ApiError::Endpoint)?;
        api.register(users::tokens::get_one)
            .map_err(ApiError::Endpoint)?;

        Ok(())
    }
}
