use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseCreated};
use bencher_json::{JsonNewRun, JsonReport, ProjectSlug, ResourceName, RunContext};
use bencher_rbac::project::Permission;
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    error::{bad_request_error, unauthorized_error},
    model::{
        project::{QueryProject, report::QueryReport},
        user::public::{PubBearerToken, PublicUser},
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
    let public_user = PublicUser::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;

    let json = post_inner(&rqctx.log, rqctx.context(), &public_user, body.into_inner()).await?;

    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    public_user: &PublicUser,
    json_run: JsonNewRun,
) -> Result<JsonReport, HttpError> {
    let user_id = match public_user {
        PublicUser::Public(remote_ip) => {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunUnclaimed);

            if let Some(remote_ip) = remote_ip {
                slog::info!(log, "Unclaimed run request from remote IP address"; "remote_ip" => ?remote_ip);
                #[cfg(feature = "plus")]
                context.rate_limiting.unclaimed_run(*remote_ip)?;
            }

            None
        },
        PublicUser::Auth(auth_user) => {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunClaimed);

            #[cfg(feature = "plus")]
            context.rate_limiting.claimed_run(auth_user.user.uuid)?;

            Some(auth_user.id)
        },
    };

    let project_name_fn = || project_name(&json_run);
    let project_slug_fn = || project_slug(&json_run);
    let query_project = if let Some(project) = json_run.project.as_ref() {
        QueryProject::get_or_create(log, context, public_user, project, project_name_fn).await?
    } else {
        QueryProject::get_or_create_from_context(
            log,
            context,
            public_user,
            project_name_fn,
            project_slug_fn,
        )
        .await?
    };

    let query_organization = query_project.organization(conn_lock!(context))?;
    let is_claimed = query_organization.is_claimed(conn_lock!(context))?;
    // If the organization is claimed, check permissions
    if is_claimed {
        match public_user {
            PublicUser::Public(remote_ip) => {
                slog::info!(log, "Public user attempted to create a run for a claimed project"; "project" => ?query_project.uuid, "remote_ip" => ?remote_ip);

                return Err(unauthorized_error(format!(
                    "This project ({slug}) has already been claimed. Provide a valid API token (`--token`) to authenticate.",
                    slug = query_project.slug
                )));
            },
            PublicUser::Auth(auth_user) => {
                // If the user is authenticated, then we may have created a new role for them.
                // If so then we need to reload the permissions.
                let auth_user = auth_user.reload(conn_lock!(context))?;
                query_project.try_allowed(&context.rbac, &auth_user, Permission::Create)?;
            },
        }
    // If the organization is not claimed and the user is authenticated, claim it
    } else if let PublicUser::Auth(auth_user) = public_user {
        query_organization.claim(context, &auth_user.user).await?;
    }

    slog::info!(log, "New run requested"; "project" => ?query_project, "run" => ?json_run);
    QueryReport::create(log, context, &query_project, json_run.into(), user_id).await
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
