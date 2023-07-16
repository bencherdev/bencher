use bencher_json::{
    project::threshold::{JsonNewThreshold, JsonThreshold, JsonUpdateThreshold},
    JsonDirection, JsonEmpty, JsonPagination, JsonThresholds, ResourceId,
};
use bencher_rbac::project::Permission;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
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
        metric_kind::QueryMetricKind,
        threshold::{
            statistic::{InsertStatistic, QueryStatistic},
            InsertThreshold, QueryThreshold, UpdateThreshold,
        },
        QueryProject,
    },
    model::user::auth::AuthUser,
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::into_json,
        same_project::SameProject,
    },
    ApiError,
};

pub mod alerts;
pub mod statistics;

use super::Resource;

const THRESHOLD_RESOURCE: Resource = Resource::Threshold;

#[derive(Deserialize, JsonSchema)]
pub struct ProjThresholdsParams {
    pub project: ResourceId,
}

pub type ProjThresholdsQuery = JsonPagination<ProjThresholdsSort, ()>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjThresholdsSort {
    #[default]
    Created,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/thresholds",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_thresholds_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjThresholdsParams>,
    _query_params: Query<ProjThresholdsQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/thresholds",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_thresholds_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjThresholdsParams>,
    query_params: Query<ProjThresholdsQuery>,
) -> Result<ResponseOk<JsonThresholds>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(THRESHOLD_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        path_params.into_inner(),
        query_params.into_inner(),
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
    path_params: ProjThresholdsParams,
    query_params: ProjThresholdsQuery,
    endpoint: Endpoint,
) -> Result<JsonThresholds, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    let mut query = schema::threshold::table
        .filter(schema::threshold::project_id.eq(query_project.id))
        .into_boxed();

    query = match query_params.order() {
        ProjThresholdsSort::Created => match query_params.direction {
            Some(JsonDirection::Asc) => query.order(schema::threshold::created.asc()),
            Some(JsonDirection::Desc) | None => query.order(schema::threshold::created.desc()),
        },
    };

    Ok(query
        .offset(query_params.offset())
        .limit(query_params.limit())
        .load::<QueryThreshold>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(into_json!(endpoint, conn))
        .collect())
}

#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/thresholds",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_threshold_post(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjThresholdsParams>,
    body: TypedBody<JsonNewThreshold>,
) -> Result<ResponseAccepted<JsonThreshold>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(THRESHOLD_RESOURCE, Method::Post);

    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        &body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &ApiContext,
    path_params: ProjThresholdsParams,
    json_threshold: &JsonNewThreshold,
    auth_user: &AuthUser,
) -> Result<JsonThreshold, ApiError> {
    let conn = &mut *context.conn().await;

    // Verify that the branch and testbed are part of the same project
    let SameProject {
        project_id,
        branch_id,
        testbed_id,
    } = SameProject::validate(
        conn,
        &path_params.project,
        &json_threshold.branch,
        &json_threshold.testbed,
    )?;
    let metric_kind_id =
        QueryMetricKind::from_resource_id(conn, project_id, &json_threshold.metric_kind)?.id;

    // Verify that the user is allowed
    QueryProject::is_allowed_id(
        conn,
        &context.rbac,
        project_id,
        auth_user,
        Permission::Create,
    )?;

    // Create the new threshold
    let insert_threshold = InsertThreshold::new(project_id, metric_kind_id, branch_id, testbed_id);
    diesel::insert_into(schema::threshold::table)
        .values(&insert_threshold)
        .execute(conn)
        .map_err(api_error!())?;

    // Get the new threshold
    let new_threshold = schema::threshold::table
        .filter(schema::threshold::uuid.eq(&insert_threshold.uuid))
        .first::<QueryThreshold>(conn)
        .map_err(api_error!())?;

    // Create the new statistic
    let insert_statistic = InsertStatistic::from_json(new_threshold.id, json_threshold.statistic)?;
    diesel::insert_into(schema::statistic::table)
        .values(&insert_statistic)
        .execute(conn)
        .map_err(api_error!())?;

    // Get the new threshold statistic
    let new_statistic = schema::statistic::table
        .filter(schema::statistic::uuid.eq(&insert_statistic.uuid))
        .first::<QueryStatistic>(conn)?;

    // Set the new statistic for the new threshold
    diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(new_threshold.id)))
        .set(schema::threshold::statistic_id.eq(new_statistic.id))
        .execute(conn)
        .map_err(api_error!())?;

    // Return the new threshold with the new statistic
    schema::threshold::table
        .filter(schema::threshold::id.eq(new_threshold.id))
        .first::<QueryThreshold>(conn)
        .map_err(api_error!())?
        .into_json(conn)
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjThresholdParams {
    pub project: ResourceId,
    pub threshold: Uuid,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/thresholds/{threshold}",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_threshold_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjThresholdParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/thresholds/{threshold}",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_threshold_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjThresholdParams>,
) -> Result<ResponseOk<JsonThreshold>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(THRESHOLD_RESOURCE, Method::GetOne);

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
    path_params: ProjThresholdParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonThreshold, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    schema::threshold::table
        .filter(schema::threshold::project_id.eq(query_project.id))
        .filter(schema::threshold::uuid.eq(path_params.threshold.to_string()))
        .first::<QueryThreshold>(conn)
        .map_err(api_error!())?
        .into_json(conn)
}

#[endpoint {
    method = PUT,
    path =  "/v0/projects/{project}/thresholds/{threshold}",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_threshold_put(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjThresholdParams>,
    body: TypedBody<JsonUpdateThreshold>,
) -> Result<ResponseAccepted<JsonThreshold>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(THRESHOLD_RESOURCE, Method::Patch);

    let context = rqctx.context();
    let json = put_inner(
        context,
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn put_inner(
    context: &ApiContext,
    path_params: ProjThresholdParams,
    json_threshold: JsonUpdateThreshold,
    auth_user: &AuthUser,
) -> Result<JsonThreshold, ApiError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed_resource_id(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Edit,
    )?;

    // Get the current threshold
    let query_threshold = schema::threshold::table
        .filter(schema::threshold::project_id.eq(query_project.id))
        .filter(schema::threshold::uuid.eq(path_params.threshold.to_string()))
        .first::<QueryThreshold>(conn)
        .map_err(api_error!())?;

    // Insert the new statistic
    let insert_statistic = InsertStatistic::from_json(query_project.id, json_threshold.statistic)?;
    diesel::insert_into(schema::statistic::table)
        .values(&insert_statistic)
        .execute(conn)
        .map_err(api_error!())?;

    // Update the current threshold to use the new statistic
    diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(query_threshold.id)))
        .set(&UpdateThreshold::new_statistic(
            conn,
            &insert_statistic.uuid,
        )?)
        .execute(conn)
        .map_err(api_error!())?;

    QueryThreshold::get(conn, query_threshold.id)?.into_json(conn)
}

#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/thresholds/{threshold}",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_threshold_delete(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjThresholdParams>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(THRESHOLD_RESOURCE, Method::Delete);

    let json = delete_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjThresholdParams,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, ApiError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed_resource_id(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Delete,
    )?;

    let query_threshold = schema::threshold::table
        .filter(schema::threshold::project_id.eq(query_project.id))
        .filter(schema::threshold::uuid.eq(path_params.threshold.to_string()))
        .first::<QueryThreshold>(conn)
        .map_err(api_error!())?;
    diesel::delete(schema::threshold::table.filter(schema::threshold::id.eq(query_threshold.id)))
        .execute(conn)
        .map_err(api_error!())?;

    Ok(JsonEmpty {})
}
