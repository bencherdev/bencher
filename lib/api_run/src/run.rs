use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseCreated};
use bencher_json::{project, system::auth, JsonNewRun, JsonReport, NameIdKind, ResourceName};
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    error::{bad_request_error, forbidden_error},
    model::{
        organization::QueryOrganization,
        project::{report::QueryReport, QueryProject},
        user::auth::{AuthUser, PubBearerToken},
    },
};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use slog::Logger;

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
    let query_project = QueryProject::get_or_create(
        context,
        json_run.organization.as_ref(),
        json_run.project.as_ref(),
        auth_user.as_ref(),
    )
    .await?;
    #[allow(clippy::unimplemented)]
    let todo_pub_run_user = |_auth_user: Option<AuthUser>| -> Result<AuthUser, HttpError> {
        Err(bad_request_error("pub run creation is not yet implemented"))
    };
    let auth_user = todo_pub_run_user(auth_user)?;
    QueryReport::create(log, context, &query_project, json_run.into(), &auth_user).await
}
