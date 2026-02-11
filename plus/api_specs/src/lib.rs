#![cfg(feature = "plus")]

// Dev dependencies used by integration tests
#[cfg(test)]
use bencher_api_tests as _;
#[cfg(test)]
use http as _;
#[cfg(test)]
use serde_json as _;
#[cfg(test)]
use tokio as _;

mod specs;

pub struct Api;

impl bencher_endpoint::Registrar for Api {
    fn register(
        api_description: &mut dropshot::ApiDescription<bencher_schema::ApiContext>,
        http_options: bool,
        #[cfg(feature = "plus")] _is_bencher_cloud: bool,
    ) -> Result<(), dropshot::ApiDescriptionRegisterError> {
        // Spec Management (admin only)
        if http_options {
            api_description.register(specs::specs_options)?;
            api_description.register(specs::spec_options)?;
        }
        api_description.register(specs::specs_get)?;
        api_description.register(specs::specs_post)?;
        api_description.register(specs::spec_get)?;
        api_description.register(specs::spec_patch)?;
        api_description.register(specs::spec_delete)?;

        Ok(())
    }
}
