#![cfg(feature = "plus")]

// Dev dependencies used by integration tests
#[cfg(test)]
use bencher_api_tests as _;
#[cfg(test)]
use futures_concurrency as _;
#[cfg(test)]
use http as _;
#[cfg(test)]
use serde_json as _;
#[cfg(test)]
use tokio as _;

mod channel;
mod jobs;
mod runner_token;
mod runners;
mod specs;
mod token;

pub use channel::{RunnerMessage, ServerMessage};
pub use runners::RUNNER_TOKEN_LENGTH;

pub struct Api;

impl bencher_endpoint::Registrar for Api {
    fn register(
        api_description: &mut dropshot::ApiDescription<bencher_schema::ApiContext>,
        http_options: bool,
        #[cfg(feature = "plus")] _is_bencher_cloud: bool,
    ) -> Result<(), dropshot::ApiDescriptionRegisterError> {
        // Runner Management (admin only)
        if http_options {
            api_description.register(runners::runners_options)?;
            api_description.register(runners::runner_options)?;
        }
        api_description.register(runners::runners_get)?;
        api_description.register(runners::runners_post)?;
        api_description.register(runners::runner_get)?;
        api_description.register(runners::runner_patch)?;

        // Runner-Spec Associations (admin only)
        if http_options {
            api_description.register(specs::runner_specs_options)?;
            api_description.register(specs::runner_spec_options)?;
        }
        api_description.register(specs::runner_specs_get)?;
        api_description.register(specs::runner_specs_post)?;
        api_description.register(specs::runner_spec_delete)?;

        // Token Rotation (admin only)
        if http_options {
            api_description.register(token::runner_token_options)?;
        }
        api_description.register(token::runner_token_post)?;

        // Runner Agent Endpoints (runner token auth)
        if http_options {
            api_description.register(jobs::runner_jobs_options)?;
            api_description.register(jobs::runner_job_options)?;
        }
        api_description.register(jobs::runner_jobs_post)?;
        api_description.register(jobs::runner_job_patch)?;

        // WebSocket channel for job execution
        api_description.register(channel::runner_job_channel)?;

        Ok(())
    }
}
