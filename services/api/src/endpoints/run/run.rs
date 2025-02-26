use bencher_json::{JsonNewRun, JsonReport};

use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use slog::Logger;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Post, ResponseCreated},
        Endpoint,
    },
    error::bad_request_error,
    model::{
        project::{report::QueryReport, QueryProject},
        user::auth::{AuthUser, PubBearerToken},
    },
};

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/run",
    tags = ["run", "reports"]
}]
pub async fn run_options(_rqctx: RequestContext<ApiContext>) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Create a run
///
/// Create a run.
/// The user does not need have an account yet or be authenticated.
/// The project may or may not exist yet.
#[endpoint {
    method = POST,
    path =  "/v0/run",
    tags = ["run", "reports"]
}]
pub async fn run_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    body: TypedBody<JsonNewRun>,
) -> Result<ResponseCreated<JsonReport>, HttpError> {
    let auth_user = AuthUser::from_pub_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(&rqctx.log, rqctx.context(), body.into_inner(), auth_user).await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    json_run: JsonNewRun,
    auth_user: Option<AuthUser>,
) -> Result<JsonReport, HttpError> {
    #[allow(clippy::unimplemented)]
    let todo_pub_run_project = || -> Result<QueryProject, HttpError> {
        Err(bad_request_error("pub run creation is not yet implemented"))
    };
    let query_project = todo_pub_run_project()?;
    #[allow(clippy::unimplemented)]
    let todo_pub_run_user = |_auth_user: Option<AuthUser>| -> Result<AuthUser, HttpError> {
        Err(bad_request_error("pub run creation is not yet implemented"))
    };
    let auth_user = todo_pub_run_user(auth_user)?;
    QueryReport::create(log, context, &query_project, json_run.into(), &auth_user).await
}
