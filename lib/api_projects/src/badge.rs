use bencher_endpoint::{CorsResponse, Endpoint, Get};
use bencher_json::{BranchNameId, ProjectResourceId, TestbedNameId, urlencoded::from_urlencoded};
use bencher_schema::{
    actor_conn,
    context::ApiContext,
    error::{bad_request_error, issue_error, with_auth_hint},
    model::{
        project::{QueryProject, branch::QueryBranch, testbed::QueryTestbed},
        user::actor::{ApiActor, PubProjectBearerToken},
    },
};
use dropshot::{Body, HttpError, Path, Query, RequestContext, endpoint};
use http::{Response, StatusCode, header};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::alerts::has_active;

/// The badge of a project with no active alerts.
pub const PASSING_SVG: &str = include_str!("badge/passing.svg");
/// The badge of a project with at least one active alert.
pub const FAILING_SVG: &str = include_str!("badge/failing.svg");

const PUBLIC_CACHE_CONTROL: &str = "public, max-age=300, s-maxage=300";
const PRIVATE_CACHE_CONTROL: &str = "private, max-age=300";

#[derive(Deserialize, JsonSchema)]
pub struct ProjBadgeParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjBadgeQuery {
    /// The name, slug, or UUID for a branch.
    /// If set, only the alerts on this branch count.
    pub branch: Option<String>,
    /// The name, slug, or UUID for a testbed.
    /// If set, only the alerts on this testbed count.
    pub testbed: Option<String>,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/badge",
    tags = ["projects", "alerts"]
}]
pub async fn proj_badge_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjBadgeParams>,
    _query_params: Query<ProjBadgeQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// View a status badge for a project
///
/// View an SVG status badge for a project.
/// The badge reads `passing` when the project has no active alerts,
/// and `failing` when it has at least one.
/// An alert counts when it is active and its branch, testbed, and measure are not archived.
/// Use the `branch` and `testbed` query parameters to only count the alerts on a branch or testbed.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/badge",
    tags = ["projects", "alerts"]
}]
pub async fn proj_badge_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjBadgeParams>,
    query_params: Query<ProjBadgeQuery>,
) -> Result<Response<Body>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let badge = get_inner(
        rqctx.context(),
        &api_actor,
        path_params.into_inner(),
        query_params.into_inner(),
    )
    .await
    .map_err(with_auth_hint)?;

    let cache_control = if badge.is_public {
        PUBLIC_CACHE_CONTROL
    } else {
        PRIVATE_CACHE_CONTROL
    };
    let svg = if badge.failing {
        FAILING_SVG
    } else {
        PASSING_SVG
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/svg+xml")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CACHE_CONTROL, cache_control)
        .body(svg.into())
        .map_err(Into::into)
}

/// Whether a project is failing, and whether anyone may read it.
struct Badge {
    failing: bool,
    is_public: bool,
}

async fn get_inner(
    context: &ApiContext,
    api_actor: &ApiActor,
    path_params: ProjBadgeParams,
    query_params: ProjBadgeQuery,
) -> Result<Badge, HttpError> {
    let query_project = QueryProject::is_allowed_actor_pub(
        actor_conn!(context, api_actor),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        api_actor,
    )?;

    let ProjBadgeQuery { branch, testbed } = query_params;
    let branch = branch
        .as_deref()
        .map(from_urlencoded::<BranchNameId>)
        .transpose()
        .map_err(bad_request_error)?;
    let testbed = testbed
        .as_deref()
        .map(from_urlencoded::<TestbedNameId>)
        .transpose()
        .map_err(bad_request_error)?;
    actor_conn!(context, api_actor, |conn| {
        let branch_id = branch
            .map(|branch| QueryBranch::from_name_id(conn, query_project.id, &branch))
            .transpose()?
            .map(|query_branch| query_branch.id);
        let testbed_id = testbed
            .map(|testbed| QueryTestbed::from_name_id(conn, query_project.id, &testbed))
            .transpose()?
            .map(|query_testbed| query_testbed.id);
        let failing = has_active(conn, &query_project, branch_id, testbed_id).map_err(|e| {
            issue_error(
                "Failed to check for active alerts",
                &format!(
                    "Failed to check for active alerts for project ({}).",
                    query_project.uuid
                ),
                e,
            )
        })?;
        Ok(Badge {
            failing,
            is_public: query_project.is_public(),
        })
    })
}
