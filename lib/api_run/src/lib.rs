#[cfg(test)]
use bencher_api_tests as _;
#[cfg(test)]
use diesel as _;
#[cfg(test)]
use http as _;
#[cfg(test)]
use serde_json as _;
#[cfg(test)]
use tokio as _;

mod run;

pub struct Api;

impl bencher_endpoint::Registrar for Api {
    fn register(
        api_description: &mut dropshot::ApiDescription<bencher_schema::ApiContext>,
        http_options: bool,
        #[cfg(feature = "plus")] _is_bencher_cloud: bool,
    ) -> Result<(), dropshot::ApiDescriptionRegisterError> {
        // Run
        if http_options {
            api_description.register(run::run_options)?;
        }
        api_description.register(run::run_post)?;

        Ok(())
    }
}
