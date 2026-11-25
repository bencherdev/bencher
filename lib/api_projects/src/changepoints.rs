#![cfg(feature = "plus")]

use bencher_endpoint::{CorsResponse, Endpoint, Get, Patch, ResponseOk, TotalCount};
use bencher_json::{
    ChangepointUuid, JsonChangepoint, JsonChangepoints, JsonDirection, JsonPagination,
    ProjectResourceId,
    project::changepoint::{
        JsonChangepointQuery, JsonChangepointQueryParams, JsonUpdateChangepoint,
    },
};
use bencher_rbac::project::Permission;
use bencher_schema::{
    actor_conn, auth_conn,
    context::ApiContext,
    error::{bad_request_error, resource_conflict_err, resource_not_found_err, with_auth_hint},
    model::{
        project::{
            QueryProject,
            threshold::{
                alert::archived_filter,
                changepoint::{ChangepointState, QueryChangepoint},
            },
        },
        user::actor::{ApiActor, PubProjectBearerToken},
    },
    schema, write_conn,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::macros::{
    filter_benchmark_name_id, filter_branch_name_id, filter_measure_name_id, filter_testbed_name_id,
};

pub(crate) fn register(
    api_description: &mut dropshot::ApiDescription<ApiContext>,
    http_options: bool,
) -> Result<(), dropshot::ApiDescriptionRegisterError> {
    if http_options {
        api_description.register(proj_changepoints_options)?;
        api_description.register(proj_changepoint_options)?;
    }
    api_description.register(proj_changepoints_get)?;
    api_description.register(proj_changepoint_get)?;
    api_description.register(proj_changepoint_patch)?;
    Ok(())
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjChangepointsParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

pub type ProjChangepointsPagination = JsonPagination<ProjChangepointsSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjChangepointsSort {
    /// Sort by changepoint creation date time.
    Created,
    /// Sort by changepoint modified date time.
    #[default]
    Modified,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/changepoints",
    tags = ["projects", "changepoints"]
}]
pub async fn proj_changepoints_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjChangepointsParams>,
    _pagination_params: Query<ProjChangepointsPagination>,
    _query_params: Query<JsonChangepointQueryParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List changepoints for a project
///
/// List the changepoints of a project with one status, active by default.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project,
/// or provide a valid project key for the project.
/// By default, the changepoints are sorted by modification date time in reverse chronological order.
/// The HTTP response header `X-Total-Count` contains the total number of changepoints.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/changepoints",
    tags = ["projects", "changepoints"]
}]
pub async fn proj_changepoints_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjChangepointsParams>,
    pagination_params: Query<ProjChangepointsPagination>,
    query_params: Query<JsonChangepointQueryParams>,
) -> Result<ResponseOk<JsonChangepoints>, HttpError> {
    let api_actor = ApiActor::new(&rqctx).await?;
    let query_params =
        JsonChangepointQuery::try_from(query_params.into_inner()).map_err(bad_request_error)?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        &api_actor,
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params,
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Get::response_ok_with_total_count(
        json,
        api_actor.is_auth(),
        total_count,
    ))
}

pub async fn get_ls_inner(
    context: &ApiContext,
    api_actor: &ApiActor,
    path_params: ProjChangepointsParams,
    pagination_params: ProjChangepointsPagination,
    query_params: JsonChangepointQuery,
) -> Result<(JsonChangepoints, TotalCount), HttpError> {
    let query_project = QueryProject::is_allowed_actor_pub(
        actor_conn!(context, api_actor),
        &context.rbac,
        &context.rate_limiting,
        &path_params.project,
        api_actor,
    )?;

    let conn = actor_conn!(context, api_actor);
    let changepoints = get_ls_query(&query_project, &query_params, &pagination_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .select(QueryChangepoint::as_select())
        .load::<QueryChangepoint>(conn)
        .map_err(resource_not_found_err!(
            Changepoint,
            (&query_project, &pagination_params, &query_params)
        ))?;
    let json_changepoints = changepoints
        .into_iter()
        .filter_map(
            |changepoint| match changepoint.into_json_for_project(conn, &query_project) {
                Ok(changepoint) => Some(changepoint),
                Err(err) => {
                    debug_assert!(false, "{err}");
                    #[cfg(feature = "sentry")]
                    sentry::capture_error(&err);
                    None
                },
            },
        )
        .collect::<Vec<_>>();
    let total_count = get_ls_filter(&query_project, &query_params)
        .count()
        .get_result::<i64>(conn)
        .map_err(resource_not_found_err!(
            Changepoint,
            (&query_project, &pagination_params, &query_params)
        ))?
        .try_into()?;

    Ok((json_changepoints.into(), total_count))
}

/// The changepoints the list holds, with one status a user sees, so never a candidate.
fn get_ls_filter<'q>(
    query_project: &QueryProject,
    query_params: &'q JsonChangepointQuery,
) -> FilterQuery<'q> {
    let status = ChangepointState::from(query_params.status.unwrap_or_default());
    let mut query = schema::changepoint::table
        .inner_join(schema::threshold::table.inner_join(schema::branch::table))
        .inner_join(schema::testbed::table)
        .inner_join(schema::benchmark::table)
        .inner_join(schema::measure::table)
        .filter(schema::changepoint::project_id.eq(query_project.id))
        .filter(schema::changepoint::status.eq(status))
        .into_boxed();

    if let Some(branch) = query_params.branch.as_ref() {
        filter_branch_name_id!(query, branch);
    }
    if let Some(testbed) = query_params.testbed.as_ref() {
        filter_testbed_name_id!(query, testbed);
    }
    if let Some(benchmark) = query_params.benchmark.as_ref() {
        filter_benchmark_name_id!(query, benchmark);
    }
    if let Some(measure) = query_params.measure.as_ref() {
        filter_measure_name_id!(query, measure);
    }

    query.filter(archived_filter(query_params.archived))
}

fn get_ls_query<'q>(
    query_project: &QueryProject,
    query_params: &'q JsonChangepointQuery,
    pagination_params: &ProjChangepointsPagination,
) -> FilterQuery<'q> {
    let query = get_ls_filter(query_project, query_params);
    // The identifier breaks every tie, so each order is total.
    match pagination_params.order() {
        ProjChangepointsSort::Created => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order((
                schema::changepoint::created.asc(),
                schema::changepoint::id.asc(),
            )),
            Some(JsonDirection::Desc) => query.order((
                schema::changepoint::created.desc(),
                schema::changepoint::id.desc(),
            )),
        },
        ProjChangepointsSort::Modified => match pagination_params.direction {
            Some(JsonDirection::Asc) => query.order((
                schema::changepoint::modified.asc(),
                schema::changepoint::id.asc(),
            )),
            Some(JsonDirection::Desc) | None => query.order((
                schema::changepoint::modified.desc(),
                schema::changepoint::id.desc(),
            )),
        },
    }
}

type FilterQuery<'q> = diesel::dsl::IntoBoxed<
    'q,
    diesel::dsl::InnerJoin<
        diesel::dsl::InnerJoin<
            diesel::dsl::InnerJoin<
                diesel::dsl::InnerJoin<
                    schema::changepoint::table,
                    diesel::dsl::InnerJoin<schema::threshold::table, schema::branch::table>,
                >,
                schema::testbed::table,
            >,
            schema::benchmark::table,
        >,
        schema::measure::table,
    >,
    diesel::sqlite::Sqlite,
>;

#[derive(Deserialize, JsonSchema)]
pub struct ProjChangepointParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
    /// The UUID for a changepoint.
    pub changepoint: ChangepointUuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/changepoints/{changepoint}",
    tags = ["projects", "changepoints"]
}]
pub async fn proj_changepoint_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjChangepointParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into()]))
}

/// View a changepoint
///
/// View a changepoint for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/changepoints/{changepoint}",
    tags = ["projects", "changepoints"]
}]
pub async fn proj_changepoint_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjChangepointParams>,
) -> Result<ResponseOk<JsonChangepoint>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &api_actor)
        .await
        .map_err(with_auth_hint)?;
    Ok(Get::response_ok(json, api_actor.is_auth()))
}

pub async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjChangepointParams,
    api_actor: &ApiActor,
) -> Result<JsonChangepoint, HttpError> {
    let query_project = QueryProject::is_allowed_actor_pub(
        actor_conn!(context, api_actor),
        &context.rbac,
        &context.rate_limiting,
        &path_params.project,
        api_actor,
    )?;

    actor_conn!(context, api_actor, |conn| {
        QueryChangepoint::from_uuid(conn, query_project.id, path_params.changepoint)?
            .into_json_with_scans_for_project(conn, &query_project)
    })
}

/// Update a changepoint
///
/// Update the status of a changepoint for a project.
/// The user must have `edit` permissions for the project,
/// or provide a valid project key for the project.
/// Use this endpoint to dismiss or reactivate a changepoint.
/// A silenced or withdrawn changepoint cannot be changed.
#[endpoint {
    method = PATCH,
    path =  "/v0/projects/{project}/changepoints/{changepoint}",
    tags = ["projects", "changepoints"]
}]
pub async fn proj_changepoint_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjChangepointParams>,
    body: TypedBody<JsonUpdateChangepoint>,
) -> Result<ResponseOk<JsonChangepoint>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = patch_inner(
        rqctx.context(),
        &api_actor,
        path_params.into_inner(),
        body.into_inner(),
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Patch::auth_response_ok(json))
}

pub async fn patch_inner(
    context: &ApiContext,
    api_actor: &ApiActor,
    path_params: ProjChangepointParams,
    json_changepoint: JsonUpdateChangepoint,
) -> Result<JsonChangepoint, HttpError> {
    let query_project = QueryProject::is_allowed_actor_auth(
        auth_conn!(context),
        &context.rbac,
        &context.rate_limiting,
        &path_params.project,
        api_actor,
        Permission::Edit,
    )?;
    let query_changepoint = QueryChangepoint::from_uuid(
        auth_conn!(context),
        query_project.id,
        path_params.changepoint,
    )?;

    let now = context.clock.now();
    let changed = QueryChangepoint::set_status(
        write_conn!(context),
        query_changepoint.id,
        json_changepoint.status,
        now,
    )
    .map_err(resource_conflict_err!(
        Changepoint,
        (&query_changepoint, &json_changepoint)
    ))?;

    auth_conn!(context, |conn| {
        let query_changepoint = QueryChangepoint::get(conn, query_changepoint.id)?;
        if changed == 0 {
            return Err(bad_request_error(format!(
                "This changepoint is {status}, so its status cannot be changed. Only an active or a dismissed changepoint can be dismissed or made active.",
                status = state_name(query_changepoint.status)
            )));
        }
        query_changepoint.into_json_with_scans_for_project(conn, &query_project)
    })
}

fn state_name(state: ChangepointState) -> &'static str {
    match state {
        ChangepointState::Candidate => "a candidate",
        ChangepointState::Active => "active",
        ChangepointState::Dismissed => "dismissed",
        ChangepointState::Silenced => "silenced",
        ChangepointState::Withdrawn => "withdrawn",
    }
}
