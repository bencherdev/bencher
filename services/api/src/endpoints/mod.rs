use std::sync::Arc;

use dropshot::{
    endpoint,
    ApiDescription,
    ApiEndpoint,
    HttpError,
    HttpResponseHeaders,
    HttpResponseOk,
    Method,
    RequestContext,
};

pub mod adapters;
pub mod auth;
pub mod ping;
pub mod projects;
pub mod reports;
pub mod testbeds;

use crate::util::{
    headers::CorsHeaders,
    registrar::Registrar,
    Context,
};

pub struct Api;

impl Registrar<Context> for Api {
    fn register(&self, api: &mut ApiDescription<Context>) -> Result<(), String> {
        api.register(ping::api_get_ping)?;
        // Auth
        Self::register(api, auth::api_post_signup)?;
        Self::register(api, auth::api_post_login)?;
        // Projects
        api.register(projects::api_get_projects)?;
        Self::register(api, projects::api_post_project)?;
        // Testbeds
        api.register(testbeds::api_get_testbeds)?;
        api.register(testbeds::api_get_testbed)?;
        api.register(testbeds::api_post_testbed)?;
        // Adapters
        api.register(adapters::api_get_adapters)?;
        api.register(adapters::api_get_adapter)?;
        // Reports
        api.register(reports::api_get_reports)?;
        api.register(reports::api_get_report)?;
        api.register(reports::api_post_report)?;
        Ok(())
    }
}

impl Api {
    fn register<T>(api: &mut ApiDescription<Context>, endpoint: T) -> Result<(), String>
    where
        T: Into<ApiEndpoint<Context>>,
    {
        let endpoint = endpoint.into();
        let mut options_endpoint: ApiEndpoint<Context> = api_options.into();
        options_endpoint.method = Method::OPTIONS;
        options_endpoint.path = endpoint.path.clone();
        api.register(options_endpoint)?;
        api.register(endpoint)?;
        Ok(())
    }
}
#[endpoint {
    method = GET,
    path = "/v0",
}]
pub async fn api_options(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<()>, CorsHeaders>, HttpError> {
    Ok(HttpResponseHeaders::new(
        HttpResponseOk(()),
        CorsHeaders::new_origin_all("OPTIONS".into(), "Content-Type, Authorization".into(), None),
    ))
}
