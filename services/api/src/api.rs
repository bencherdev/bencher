use bencher_endpoint::Registrar;
use bencher_schema::context::ApiContext;
use dropshot::{ApiDescription, ApiDescriptionRegisterError};

pub struct Api;

impl Registrar for Api {
    fn register(
        api_description: &mut ApiDescription<ApiContext>,
        http_options: bool,
        #[cfg(feature = "plus")] is_bencher_cloud: bool,
    ) -> Result<(), ApiDescriptionRegisterError> {
        api_auth::Api::register(
            api_description,
            http_options,
            #[cfg(feature = "plus")]
            is_bencher_cloud,
        )?;
        #[cfg(feature = "plus")]
        api_checkout::Api::register(api_description, http_options, is_bencher_cloud)?;
        api_organizations::Api::register(
            api_description,
            http_options,
            #[cfg(feature = "plus")]
            is_bencher_cloud,
        )?;
        api_projects::Api::register(
            api_description,
            http_options,
            #[cfg(feature = "plus")]
            is_bencher_cloud,
        )?;
        api_run::Api::register(
            api_description,
            http_options,
            #[cfg(feature = "plus")]
            is_bencher_cloud,
        )?;
        api_server::Api::register(
            api_description,
            http_options,
            #[cfg(feature = "plus")]
            is_bencher_cloud,
        )?;
        api_users::Api::register(
            api_description,
            http_options,
            #[cfg(feature = "plus")]
            is_bencher_cloud,
        )?;

        // OCI Registry (Plus feature)
        #[cfg(feature = "plus")]
        bencher_oci::Api::register(api_description, http_options, is_bencher_cloud)?;

        Ok(())
    }
}
