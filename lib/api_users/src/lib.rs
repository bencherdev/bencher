// Dev dependencies used by integration tests
#[cfg(test)]
use bencher_api_tests as _;
#[cfg(test)]
use http as _;
#[cfg(test)]
use serde_json as _;
#[cfg(test)]
use tokio as _;

mod tokens;
mod keys;
mod users;

pub struct Api;

impl bencher_endpoint::Registrar for Api {
    fn register(
        api_description: &mut dropshot::ApiDescription<bencher_schema::ApiContext>,
        http_options: bool,
        #[cfg(feature = "plus")] _is_bencher_cloud: bool,
    ) -> Result<(), dropshot::ApiDescriptionRegisterError> {
        // Users
        if http_options {
            api_description.register(users::users_options)?;
            api_description.register(users::user_options)?;
        }
        api_description.register(users::users_get)?;
        api_description.register(users::user_get)?;
        api_description.register(users::user_patch)?;

        // Keys
        if http_options {
            api_description.register(keys::user_keys_options)?;
            api_description.register(keys::user_key_options)?;
        }
        api_description.register(keys::user_keys_get)?;
        api_description.register(keys::user_key_post)?;
        api_description.register(keys::user_key_get)?;
        api_description.register(keys::user_key_patch)?;
        api_description.register(keys::user_key_delete)?;

        // Tokens
        if http_options {
            api_description.register(tokens::user_tokens_options)?;
            api_description.register(tokens::user_token_options)?;
        }
        api_description.register(tokens::user_tokens_get)?;
        api_description.register(tokens::user_token_post)?;
        api_description.register(tokens::user_token_get)?;
        api_description.register(tokens::user_token_patch)?;
        api_description.register(tokens::user_token_delete)?;

        Ok(())
    }
}
