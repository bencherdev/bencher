use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseCreated};
use bencher_json::{JsonNewRun, JsonReport, ResourceName, RunContext, Slug};
use bencher_rbac::project::Permission;
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    error::{bad_request_error, unauthorized_error},
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
    let project_name_fn = || project_name(&json_run);
    let project_slug_fn = || project_slug(&json_run);
    let query_project = if let Some(project) = json_run.project.as_ref() {
        QueryProject::get_or_create(log, context, auth_user.as_ref(), project, project_name_fn)
            .await?
    } else {
        QueryProject::get_or_create_from_context(
            log,
            context,
            auth_user.as_ref(),
            project_name_fn,
            project_slug_fn,
        )
        .await?
    };

    // If a project is unclaimed, don't check permissions
    if !query_project.is_unclaimed(conn_lock!(context))? {
        if let Some(auth_user) = auth_user.as_ref() {
            query_project.try_allowed(&context.rbac, auth_user, Permission::Create)?;
        } else {
            return Err(unauthorized_error(format!(
                "This project ({}) has already been claimed.",
                query_project.slug
            )));
        }
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
