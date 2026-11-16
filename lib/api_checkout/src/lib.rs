// Dev dependencies used by integration tests
#[cfg(test)]
use bencher_api_tests as _;
#[cfg(test)]
use http as _;
#[cfg(test)]
use serde_json as _;
#[cfg(test)]
use tokio as _;

mod checkout;

pub struct Api;

#[cfg(feature = "plus")]
impl bencher_endpoint::Registrar for Api {
    fn register(
        api_description: &mut dropshot::ApiDescription<bencher_schema::ApiContext>,
        http_options: bool,
        is_bencher_cloud: bool,
    ) -> Result<(), dropshot::ApiDescriptionRegisterError> {
        // Checkout
        // Bencher Cloud only
        if is_bencher_cloud {
            if http_options {
                api_description.register(checkout::checkouts_options)?;
            }
            api_description.register(checkout::checkouts_post)?;
        }

        Ok(())
    }
}
