use bencher_json::{
    project::alert::JsonUpdateAlert, JsonAlert, JsonAlerts, JsonDirection, JsonPagination,
    ResourceId,
};
use bencher_rbac::project::Permission;
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{pub_response_ok, response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::project::{
        threshold::alert::{QueryAlert, UpdateAlert},
        QueryProject,
    },
    model::user::auth::AuthUser,
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::into_json,
    },
    ApiError,
};

use super::Resource;

const ALERT_RESOURCE: Resource = Resource::Alert;

#[derive(Deserialize, JsonSchema)]
pub struct ProjAlertsParams {
    pub project: ResourceId,
}

pub type ProjAlertsPagination = JsonPagination<ProjAlertsSort>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjAlertsSort {
    #[default]
    Created,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/alerts",
    tags = ["projects", "alerts"]
}]
pub async fn proj_alerts_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjAlertsParams>,
    _pagination_params: Query<ProjAlertsPagination>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/alerts",
    tags = ["projects", "alerts"]
}]
pub async fn proj_alerts_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjAlertsParams>,
    pagination_params: Query<ProjAlertsPagination>,
) -> Result<ResponseOk<JsonAlerts>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(ALERT_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        endpoint,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    if auth_user.is_some() {
        response_ok!(endpoint, json)
    } else {
        pub_response_ok!(endpoint, json)
    }
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: Option<&AuthUser>,
    path_params: ProjAlertsParams,
    pagination_params: ProjAlertsPagination,
    endpoint: Endpoint,
) -> Result<JsonAlerts, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    let mut query = schema::alert::table
        .left_join(schema::boundary::table.on(schema::alert::boundary_id.eq(schema::boundary::id)))
        .left_join(schema::metric::table.on(schema::metric::id.eq(schema::boundary::metric_id)))
        .left_join(schema::perf::table.on(schema::metric::perf_id.eq(schema::perf::id)))
        .left_join(
            schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
        )
        .filter(schema::benchmark::project_id.eq(query_project.id))
        .left_join(schema::report::table.on(schema::perf::report_id.eq(schema::report::id)))
        .select((
            schema::alert::id,
            schema::alert::uuid,
            schema::alert::boundary_id,
            schema::alert::boundary_limit,
            schema::alert::status,
            schema::alert::modified,
        ))
        .into_boxed();

    query = match pagination_params.order() {
        ProjAlertsSort::Created => match pagination_params.direction {
            Some(JsonDirection::Asc) => query.order((
                schema::alert::status.asc(),
                schema::report::start_time.asc(),
                schema::benchmark::name.asc(),
                schema::perf::iteration.asc(),
            )),
            Some(JsonDirection::Desc) | None => query.order((
                schema::alert::status.asc(),
                schema::report::start_time.desc(),
                schema::benchmark::name.asc(),
                schema::perf::iteration.asc(),
            )),
        },
    };

    Ok(query
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryAlert>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(into_json!(endpoint, conn))
        .collect())
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjAlertParams {
    pub project: ResourceId,
    pub alert: Uuid,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/alerts/{alert}",
    tags = ["projects", "alerts"]
}]
pub async fn proj_alert_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjAlertParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/alerts/{alert}",
    tags = ["projects", "alerts"]
}]
pub async fn proj_alert_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjAlertParams>,
) -> Result<ResponseOk<JsonAlert>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(ALERT_RESOURCE, Method::GetOne);

    let json = get_one_inner(
        rqctx.context(),
        path_params.into_inner(),
        auth_user.as_ref(),
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    if auth_user.is_some() {
        response_ok!(endpoint, json)
    } else {
        pub_response_ok!(endpoint, json)
    }
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjAlertParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonAlert, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    QueryAlert::from_uuid(conn, query_project.id, path_params.alert)?.into_json(conn)
}

#[endpoint {
    method = PATCH,
    path =  "/v0/projects/{project}/alerts/{alert}",
    tags = ["projects", "alerts"]
}]
pub async fn proj_alert_patch(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjAlertParams>,
    body: TypedBody<JsonUpdateAlert>,
) -> Result<ResponseAccepted<JsonAlert>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(ALERT_RESOURCE, Method::Patch);

    let context = rqctx.context();
    let json = patch_inner(
        context,
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn patch_inner(
    context: &ApiContext,
    path_params: ProjAlertParams,
    json_alert: JsonUpdateAlert,
    auth_user: &AuthUser,
) -> Result<JsonAlert, ApiError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed_resource_id(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Edit,
    )?;

    let query_alert = QueryAlert::from_uuid(conn, query_project.id, path_params.alert)?;
    diesel::update(schema::alert::table.filter(schema::alert::id.eq(query_alert.id)))
        .set(&UpdateAlert::from(json_alert))
        .execute(conn)
        .map_err(api_error!())?;

    QueryAlert::get(conn, query_alert.id)?.into_json(conn)
}
