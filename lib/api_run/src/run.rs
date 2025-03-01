use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseCreated};
use bencher_json::{project, system::auth, JsonNewRun, JsonReport, NameIdKind, ResourceName};
use bencher_rbac::project::Permission;
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    error::{bad_request_error, forbidden_error, issue_error},
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
    let json = post_inner(&rqctx.log, rqctx.context(), auth_user, body.into_inner()).await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    auth_user: Option<AuthUser>,
    json_run: JsonNewRun,
) -> Result<JsonReport, HttpError> {
    let (auth_user, query_project) = match (
        auth_user,
        json_run.organization.as_ref(),
        json_run.project.as_ref(),
    ) {
        (Some(auth_user), Some(organization), Some(project)) => {
            let query_project = QueryProject::get_or_create_organization_project(
                log,
                context,
                &auth_user,
                organization,
                project,
            )
            .await
            .map_err(|e| forbidden_error(e.to_string()))?;
            (auth_user, query_project)
        },
        (Some(auth_user), Some(organization), None) => return Err(bad_request_error("todo")),
        _ => return Err(bad_request_error("Not yet supported")),
    };

    // Verify that the user is allowed
    // This should always succeed if the logic above is correct
    query_project
        .try_allowed(&context.rbac, &auth_user, Permission::Create)
        .map_err(|e| {
            issue_error(
                "Failed to check run permissions",
                "Failed check the run permissions before creating a report",
                e,
            )
        })?;
    QueryReport::create(log, context, &query_project, json_run.into(), &auth_user).await
}
