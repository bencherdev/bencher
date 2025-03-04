use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseCreated};
use bencher_json::{JsonNewRun, JsonReport, ResourceName, RunContext, Slug};
use bencher_rbac::project::Permission;
use bencher_schema::{
    context::ApiContext,
    error::{bad_request_error, forbidden_error, issue_error},
    model::{
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
    let json = post_inner(&rqctx.log, rqctx.context(), auth_user, body.into_inner()).await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    auth_user: Option<AuthUser>,
    json_run: JsonNewRun,
) -> Result<JsonReport, HttpError> {
    let query_project = match (auth_user.as_ref(), json_run.project.as_ref()) {
        (Some(auth_user), Some(project)) => {
            QueryProject::get_or_create(log, context, auth_user, project)
                .await
                .map_err(|e| forbidden_error(e.to_string()))?
        },
        (Some(auth_user), None) => {
            let project_name = project_name(&json_run)?;
            let project_slug = project_slug(&json_run)?;
            QueryProject::get_or_create_from_context(
                log,
                context,
                auth_user,
                project_name,
                project_slug,
            )
            .await
            .map_err(|e| forbidden_error(e.to_string()))?
        },
        _ => return Err(bad_request_error("Not yet supported")),
    };

    // Verify that the user is allowed
    // This should always succeed if the logic above is correct
    if let Some(auth_user) = auth_user.as_ref() {
        query_project
            .try_allowed(&context.rbac, auth_user, Permission::Create)
            .map_err(|e| {
                issue_error(
                    "Failed to check run permissions",
                    "Failed check the run permissions before creating a report",
                    e,
                )
            })?;
    }
    QueryReport::create(
        log,
        context,
        &query_project,
        json_run.into(),
        auth_user.as_ref(),
    )
    .await
}

fn project_name(json_run: &JsonNewRun) -> Result<ResourceName, HttpError> {
    json_run
        .context
        .as_ref()
        .and_then(RunContext::name)
        .ok_or_else(|| {
            bad_request_error(
            "The `project` field was not specified nor was a run `context` provided for the name",
        )
        })
}

fn project_slug(json_run: &JsonNewRun) -> Result<Slug, HttpError> {
    json_run
        .context
        .as_ref()
        .map(RunContext::slug)
        .ok_or_else(|| {
            bad_request_error(
            "The `project` field was not specified nor was a run `context` provided for the slug",
        )
        })
}
