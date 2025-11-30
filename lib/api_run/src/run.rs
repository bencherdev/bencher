use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseCreated};
use bencher_json::{JsonNewRun, JsonReport, ProjectSlug, ResourceName, RunContext};
use bencher_rbac::project::Permission;
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    error::{bad_request_error, unauthorized_error},
    model::{
        project::{QueryProject, report::QueryReport},
        user::auth::{AuthUser, PubBearerToken},
    },
};
use dropshot::{HttpError, RequestContext, TypedBody, endpoint};
use slog::Logger;

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
    let auth_user = AuthUser::from_pub_token(
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        rqctx.context(),
        bearer_token,
    )
    .await?;
    let json = post_inner(
        &rqctx.log,
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        rqctx.context(),
        auth_user,
        body.into_inner(),
    )
    .await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    log: &Logger,
    #[cfg(feature = "plus")] headers: &bencher_schema::HeaderMap,
    context: &ApiContext,
    auth_user: Option<AuthUser>,
    json_run: JsonNewRun,
) -> Result<JsonReport, HttpError> {
    #[cfg(feature = "plus")]
    if let Some(auth_user) = auth_user.as_ref() {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunClaimed);

        context.rate_limiting.claimed_run(auth_user.user.uuid)?;
    } else {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunUnclaimed);

        if let Some(remote_ip) = bencher_schema::RateLimiting::remote_ip(headers) {
            slog::info!(log, "Unclaimed run request from remote IP address"; "remote_ip" => ?remote_ip);
            context.rate_limiting.unclaimed_run(remote_ip)?;
        }
    }

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

    let query_organization = query_project.organization(conn_lock!(context))?;
    let is_claimed = query_organization.is_claimed(conn_lock!(context))?;
    // If the organization is claimed, check permissions
    if is_claimed {
        if let Some(auth_user) = auth_user.as_ref() {
            // If the user is authenticated, then we may have created a new role for them.
            // If so then we need to reload the permissions.
            let auth_user = auth_user.reload(conn_lock!(context))?;
            query_project.try_allowed(&context.rbac, &auth_user, Permission::Create)?;
        } else {
            return Err(unauthorized_error(format!(
                "This project ({}) has already been claimed. Provide a valid API token (`--token`) to authenticate.",
                query_project.slug
            )));
        }
    // If the organization is not claimed and the user is authenticated, claim it
    } else if let Some(auth_user) = auth_user.as_ref() {
        query_organization.claim(context, &auth_user.user).await?;
    }

    slog::info!(log, "New run requested"; "project" => ?query_project, "run" => ?json_run);
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

fn project_slug(json_run: &JsonNewRun) -> Result<ProjectSlug, HttpError> {
    json_run
        .context
        .as_ref()
        .and_then(RunContext::slug)
        .map(Into::into)
        .ok_or_else(|| {
            bad_request_error(
            "The `project` field was not specified nor was a run `context` provided for the slug",
        )
        })
}
