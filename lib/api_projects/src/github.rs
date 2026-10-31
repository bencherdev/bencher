#![cfg(feature = "plus")]

use bencher_endpoint::{
    CorsResponse, Delete, Endpoint, Get, Post, Put, ResponseAccepted, ResponseDeleted, ResponseOk,
};
use bencher_github_client::{
    GitHubApp, GitHubClient, GitHubInstallationPermission, GitHubRepository, GitHubWebhookError,
};
use bencher_json::{
    ProjectResourceId, ResourceId,
    project::github::{
        JsonGitHubLinkUrls, JsonGitHubRepositories, JsonGitHubRepository, JsonLinkedRepository,
        JsonNewLinkedRepository, JsonProjectGitHub,
    },
    system::auth::JsonOAuth,
};
use bencher_rbac::project::Permission;
use bencher_schema::{
    HeaderMap, auth_conn,
    context::ApiContext,
    error::{
        bad_request_error, forbidden_error, issue_error, not_found_error, resource_conflict_err,
        resource_not_found_err, unauthorized_error, with_token_hint,
    },
    model::{
        project::{QueryProject, linked_repository::QueryLinkedRepository},
        user::auth::{AuthUser, BearerToken},
    },
    write_conn, write_transaction,
};
use dropshot::{HttpError, Path, RequestContext, TypedBody, UntypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;
use url::Url;

/// How long a trip to GitHub, and each grant it returns, lasts.
const GITHUB_TTL: u32 = 10 * 60;
/// The console's GitHub return page, which is also the app's first callback URL.
const GITHUB_RETURN_PATH: &str = "/console/github";
const NO_APP: &str = "This server has no GitHub App, so a project cannot link a GitHub repository. Pass a token with contents write to `--ci-callback-token` instead.";
const SIGNATURE_HEADER: &str = "x-hub-signature-256";
const EVENT_HEADER: &str = "x-github-event";

#[derive(Deserialize, JsonSchema)]
pub struct ProjGitHubParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/github",
    tags = ["projects", "github"]
}]
pub async fn proj_github_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjGitHubParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Put.into(), Delete.into()]))
}

/// View a project's linked repository
///
/// View the GitHub repository a project's dispatches go to, if it has one.
/// The user must have `manage` permissions for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/github",
    tags = ["projects", "github"]
}]
pub async fn proj_github_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjGitHubParams>,
) -> Result<ResponseOk<JsonProjectGitHub>, HttpError> {
    github_app(rqctx.context())?;
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(with_token_hint)?;
    Ok(Get::auth_response_ok(json))
}

async fn get_inner(
    context: &ApiContext,
    path_params: ProjGitHubParams,
    auth_user: &AuthUser,
) -> Result<JsonProjectGitHub, HttpError> {
    let query_project = is_allowed(context, &path_params.project, auth_user).await?;
    let linked_repository = auth_conn!(context, |conn| {
        QueryLinkedRepository::get(conn, query_project.id)
            .map_err(resource_not_found_err!(LinkedRepository, query_project))?
            .map(|link| link.into_json(conn))
            .transpose()?
    });
    Ok(JsonProjectGitHub { linked_repository })
}

/// Link a repository to a project
///
/// Link the GitHub repository that a grant from the GitHub code exchange names.
/// The grant must be for this project and from this user, and it lasts ten minutes.
/// A project links at most one repository, so this replaces any link it had.
/// The user must have `manage` permissions for the project.
#[endpoint {
    method = PUT,
    path =  "/v0/projects/{project}/github",
    tags = ["projects", "github"]
}]
pub async fn proj_github_put(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjGitHubParams>,
    body: TypedBody<JsonNewLinkedRepository>,
) -> Result<ResponseOk<JsonLinkedRepository>, HttpError> {
    github_app(rqctx.context())?;
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = put_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(with_token_hint)?;
    Ok(Put::auth_response_ok(json))
}

async fn put_inner(
    context: &ApiContext,
    path_params: ProjGitHubParams,
    json_link: JsonNewLinkedRepository,
    auth_user: &AuthUser,
) -> Result<JsonLinkedRepository, HttpError> {
    let query_project = is_allowed(context, &path_params.project, auth_user).await?;
    let now = context.clock.now();
    let grant = context
        .token_key
        .validate_github_grant(&json_link.grant, now)
        .map_err(|e| bad_request_error(format!("The GitHub grant is not valid: {e}")))?;
    if grant.project != query_project.uuid {
        return Err(forbidden_error("This GitHub grant is for another project."));
    }
    // The grant stands for its user's push access, which no other user may borrow.
    if grant.sub != auth_user.user.uuid {
        return Err(forbidden_error("This GitHub grant is for another user."));
    }

    let link = QueryLinkedRepository::connected(
        query_project.id,
        grant.installation,
        grant.repository,
        grant.full_name,
        auth_user.user.id,
        now,
    );
    link.replace(write_conn!(context))
        .map_err(resource_conflict_err!(LinkedRepository, query_project))?;
    link.into_json(auth_conn!(context))
}

/// Unlink a project's repository
///
/// Remove the project's linked repository, if it has one.
/// The user must have `manage` permissions for the project.
#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/github",
    tags = ["projects", "github"]
}]
pub async fn proj_github_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjGitHubParams>,
) -> Result<ResponseDeleted, HttpError> {
    github_app(rqctx.context())?;
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(with_token_hint)?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjGitHubParams,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    let query_project = is_allowed(context, &path_params.project, auth_user).await?;
    QueryLinkedRepository::delete(write_conn!(context), query_project.id)
        .map_err(resource_conflict_err!(LinkedRepository, query_project))?;
    Ok(())
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/github/urls",
    tags = ["projects", "github"]
}]
pub async fn proj_github_urls_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjGitHubParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// Start linking a repository to a project
///
/// The GitHub pages that start a link: the authorize page, and the install page for a
/// repository whose owner has not installed the app yet.
/// Both carry a state for this project and this user, which lasts ten minutes.
/// The user must have `manage` permissions for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/github/urls",
    tags = ["projects", "github"]
}]
pub async fn proj_github_urls_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjGitHubParams>,
) -> Result<ResponseOk<JsonGitHubLinkUrls>, HttpError> {
    let (github_client, github_app) = github(rqctx.context())?;
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = urls_inner(
        rqctx.context(),
        github_client,
        github_app,
        path_params.into_inner(),
        &auth_user,
    )
    .await
    .map_err(with_token_hint)?;
    Ok(Get::auth_response_ok(json))
}

async fn urls_inner(
    context: &ApiContext,
    github_client: &GitHubClient,
    github_app: &GitHubApp,
    path_params: ProjGitHubParams,
    auth_user: &AuthUser,
) -> Result<JsonGitHubLinkUrls, HttpError> {
    let query_project = is_allowed(context, &path_params.project, auth_user).await?;
    let state = context
        .token_key
        .new_github_state(
            context.clock.now(),
            GITHUB_TTL,
            auth_user.user.uuid,
            query_project.uuid,
        )
        .map_err(|e| {
            issue_error(
                "Failed to create a GitHub link state",
                &format!(
                    "Failed to create a GitHub link state for project ({}).",
                    query_project.uuid
                ),
                e,
            )
        })?;
    Ok(JsonGitHubLinkUrls {
        install: github_app.install_url(&state),
        authorize: github_client.auth_url_with_redirect(state, return_url(context)?),
    })
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/github/exchange",
    tags = ["projects", "github"]
}]
pub async fn github_exchange_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Exchange a GitHub code for repositories to link
///
/// Exchange the code that GitHub sent back with a link state for every repository the user can
/// push to, in every installation of the app the user can see.
/// Each repository whose installation has granted the app contents write comes with a grant
/// to link it, which lasts ten minutes.
/// The state must be from this user, and the user must have `manage` permissions for its project.
#[endpoint {
    method = POST,
    path =  "/v0/github/exchange",
    tags = ["projects", "github"]
}]
pub async fn github_exchange_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    body: TypedBody<JsonOAuth>,
) -> Result<ResponseOk<JsonGitHubRepositories>, HttpError> {
    let (github_client, _github_app) = github(rqctx.context())?;
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = exchange_inner(
        rqctx.context(),
        github_client,
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(with_token_hint)?;
    Ok(Post::auth_response_ok(json))
}

async fn exchange_inner(
    context: &ApiContext,
    github_client: &GitHubClient,
    json_oauth: JsonOAuth,
    auth_user: &AuthUser,
) -> Result<JsonGitHubRepositories, HttpError> {
    let now = context.clock.now();
    let state = context
        .token_key
        .validate_github_state(&json_oauth.state, now)
        .map_err(|e| bad_request_error(format!("The GitHub link state is not valid: {e}")))?;
    if state.sub != auth_user.user.uuid {
        return Err(forbidden_error(
            "This GitHub link was started by another user.",
        ));
    }
    let query_project = is_allowed(context, &ResourceId::Uuid(state.project), auth_user).await?;

    // A code is good for one exchange, so a failure is the client's to start over, not to retry.
    let repositories = github_client
        .oauth_repositories(json_oauth.code, return_url(context)?)
        .await
        .map_err(|e| bad_request_error(format!("Failed to read your GitHub repositories: {e}")))?
        .into_iter()
        .map(
            |GitHubRepository {
                 installation,
                 installation_url,
                 permission,
                 id,
                 full_name,
             }| {
                let grant = if permission == GitHubInstallationPermission::Granted {
                    let grant = context
                        .token_key
                        .new_github_grant(
                            now,
                            GITHUB_TTL,
                            &state,
                            installation,
                            id,
                            full_name.clone(),
                        )
                        .map_err(|e| {
                            issue_error(
                                "Failed to create a GitHub grant",
                                &format!(
                                    "Failed to create a GitHub grant for project ({}).",
                                    query_project.uuid
                                ),
                                e,
                            )
                        })?;
                    Some(grant)
                } else {
                    None
                };
                Ok(JsonGitHubRepository {
                    full_name,
                    installation,
                    installation_url,
                    permission,
                    grant,
                })
            },
        )
        .collect::<Result<_, HttpError>>()?;

    Ok(JsonGitHubRepositories {
        project: query_project.slug,
        repositories,
    })
}

/// Receive a GitHub App webhook delivery
///
/// GitHub sends the app's installation events here, signed with the app's webhook secret.
/// A server without the app accepts and ignores every delivery.
#[endpoint {
    method = POST,
    path =  "/v0/github/webhook",
    tags = ["github"],
    unpublished = true,
}]
pub async fn github_webhook_post(
    rqctx: RequestContext<ApiContext>,
    body: UntypedBody,
) -> Result<ResponseAccepted<()>, HttpError> {
    webhook_inner(rqctx.context(), rqctx.request.headers(), body.as_bytes()).await?;
    Ok(Post::pub_response_accepted(()))
}

async fn webhook_inner(
    context: &ApiContext,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<(), HttpError> {
    let Some(github_app) = &context.github_app else {
        return Ok(());
    };
    let header = |name| headers.get(name).and_then(|value| value.to_str().ok());
    let change = github_app
        .webhook(header(SIGNATURE_HEADER), header(EVENT_HEADER), body)
        .map_err(|e| match e {
            GitHubWebhookError::Signature => {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::GitHubWebhookRefused);
                unauthorized_error(e)
            },
            GitHubWebhookError::Json(_) => bad_request_error(e),
        })?;
    if let Some(change) = change {
        write_transaction!(context, |conn| QueryLinkedRepository::apply(conn, &change)).map_err(
            |e| {
                issue_error(
                    "Failed to apply a GitHub webhook delivery",
                    &format!("Failed to apply a GitHub webhook delivery ({change:?})."),
                    e,
                )
            },
        )?;
    }
    Ok(())
}

/// The app, or a refusal from a server that serves no link endpoints without one. The refusal
/// goes out without `with_token_hint`, since no token would change it.
fn github_app(context: &ApiContext) -> Result<&GitHubApp, HttpError> {
    context
        .github_app
        .as_deref()
        .ok_or_else(|| not_found_error(NO_APP))
}

fn github(context: &ApiContext) -> Result<(&GitHubClient, &GitHubApp), HttpError> {
    let github_app = github_app(context)?;
    let github_client = context
        .github_client
        .as_ref()
        .ok_or_else(|| not_found_error(NO_APP))?;
    Ok((github_client, github_app))
}

/// The project, if the user may change its settings.
async fn is_allowed(
    context: &ApiContext,
    project: &ProjectResourceId,
    auth_user: &AuthUser,
) -> Result<QueryProject, HttpError> {
    QueryProject::is_allowed(
        auth_conn!(context),
        &context.rbac,
        &context.rate_limiting,
        project,
        auth_user,
        Permission::Manage,
    )
}

fn return_url(context: &ApiContext) -> Result<Url, HttpError> {
    context.console_url.join(GITHUB_RETURN_PATH).map_err(|e| {
        issue_error(
            "Failed to build the GitHub return URL",
            &format!(
                "Failed to build the GitHub return URL from ({}).",
                context.console_url
            ),
            e,
        )
    })
}
