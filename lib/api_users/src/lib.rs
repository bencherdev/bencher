mod tokens;
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

        // Tokens
        if http_options {
            api_description.register(tokens::user_tokens_options)?;
            api_description.register(tokens::user_token_options)?;
        }
        api_description.register(tokens::user_tokens_get)?;
        api_description.register(tokens::user_token_post)?;
        api_description.register(tokens::user_token_get)?;
        api_description.register(tokens::user_token_patch)?;

        Ok(())
    }
}
