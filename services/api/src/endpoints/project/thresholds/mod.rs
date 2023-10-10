use bencher_json::{
    project::threshold::{JsonNewThreshold, JsonThreshold, JsonUpdateThreshold},
    JsonDirection, JsonEmpty, JsonPagination, JsonThresholds, ResourceId, ThresholdUuid,
};
use bencher_rbac::project::Permission;
use diesel::{BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{pub_response_ok, response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint,
    },
    model::project::{
        branch::QueryBranch,
        metric_kind::QueryMetricKind,
        testbed::QueryTestbed,
        threshold::{statistic::InsertStatistic, InsertThreshold, QueryThreshold, UpdateThreshold},
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

pub mod alerts;
pub mod statistics;

#[derive(Deserialize, JsonSchema)]
pub struct ProjThresholdsParams {
    pub project: ResourceId,
}

pub type ProjThresholdsPagination = JsonPagination<ProjThresholdsSort>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjThresholdsSort {
    Created,
    #[default]
    Modified,
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
    _pagination_params: Query<ProjThresholdsPagination>,
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
    pagination_params: Query<ProjThresholdsPagination>,
) -> Result<ResponseOk<JsonThresholds>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::GetLs;

    let json = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        endpoint,
    )
    .await
    .map_err(|e| {
        if let ApiError::HttpError(e) = e {
            e
        } else {
            endpoint.err(e).into()
        }
    })?;

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
    pagination_params: ProjThresholdsPagination,
    endpoint: Endpoint,
) -> Result<JsonThresholds, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    let mut query = QueryThreshold::belonging_to(&query_project).into_boxed();

    query = match pagination_params.order() {
        ProjThresholdsSort::Created => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::threshold::created.asc()),
            Some(JsonDirection::Desc) => query.order(schema::threshold::created.desc()),
        },
        ProjThresholdsSort::Modified => match pagination_params.direction {
            Some(JsonDirection::Asc) => query.order(schema::threshold::modified.asc()),
            Some(JsonDirection::Desc) | None => query.order(schema::threshold::modified.desc()),
        },
    };

    Ok(query
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryThreshold>(conn)
        .map_err(ApiError::from)?
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
    let endpoint = Endpoint::Post;

    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        &body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| {
        if let ApiError::HttpError(e) = e {
            e
        } else {
            endpoint.err(e).into()
        }
    })?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &ApiContext,
    path_params: ProjThresholdsParams,
    json_threshold: &JsonNewThreshold,
    auth_user: &AuthUser,
) -> Result<JsonThreshold, ApiError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let project_id = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Create,
    )?
    .id;

    // Verify that the branch, testbed, and metric kind are part of the same project
    let branch_id = QueryBranch::from_resource_id(conn, project_id, &json_threshold.branch)?.id;
    let testbed_id = QueryTestbed::from_resource_id(conn, project_id, &json_threshold.testbed)?.id;
    let metric_kind_id =
        QueryMetricKind::from_resource_id(conn, project_id, &json_threshold.metric_kind)?.id;

    // Create the new threshold
    let threshold_id = InsertThreshold::from_json(
        conn,
        project_id,
        metric_kind_id,
        branch_id,
        testbed_id,
        json_threshold.statistic,
    )?;

    // Return the new threshold with the new statistic
    schema::threshold::table
        .filter(schema::threshold::id.eq(threshold_id))
        .first::<QueryThreshold>(conn)
        .map_err(ApiError::from)?
        .into_json(conn)
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjThresholdParams {
    pub project: ResourceId,
    pub threshold: ThresholdUuid,
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
    let endpoint = Endpoint::GetOne;

    let json = get_one_inner(
        rqctx.context(),
        path_params.into_inner(),
        auth_user.as_ref(),
    )
    .await
    .map_err(|e| {
        if let ApiError::HttpError(e) = e {
            e
        } else {
            endpoint.err(e).into()
        }
    })?;

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

    QueryThreshold::belonging_to(&query_project)
        .filter(schema::threshold::uuid.eq(path_params.threshold))
        .first::<QueryThreshold>(conn)
        .map_err(ApiError::from)?
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
    let endpoint = Endpoint::Put;

    let context = rqctx.context();
    let json = put_inner(
        context,
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| {
        if let ApiError::HttpError(e) = e {
            e
        } else {
            endpoint.err(e).into()
        }
    })?;

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
    let query_project = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Edit,
    )?;

    // Get the current threshold
    let query_threshold = QueryThreshold::belonging_to(&query_project)
        .filter(schema::threshold::uuid.eq(path_params.threshold.to_string()))
        .first::<QueryThreshold>(conn)
        .map_err(ApiError::from)?;

    // Insert the new statistic
    let insert_statistic =
        InsertStatistic::from_json(query_threshold.id, json_threshold.statistic)?;
    diesel::insert_into(schema::statistic::table)
        .values(&insert_statistic)
        .execute(conn)
        .map_err(ApiError::from)?;

    // Update the current threshold to use the new statistic
    diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(query_threshold.id)))
        .set(&UpdateThreshold::new_statistic(
            conn,
            insert_statistic.uuid,
        )?)
        .execute(conn)
        .map_err(ApiError::from)?;

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
    let endpoint = Endpoint::Delete;

    let json = delete_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| {
            if let ApiError::HttpError(e) = e {
                e
            } else {
                endpoint.err(e).into()
            }
        })?;

    response_accepted!(endpoint, json)
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjThresholdParams,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, ApiError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Delete,
    )?;

    let query_threshold = QueryThreshold::belonging_to(&query_project)
        .filter(schema::threshold::uuid.eq(path_params.threshold.to_string()))
        .first::<QueryThreshold>(conn)
        .map_err(ApiError::from)?;
    diesel::delete(schema::threshold::table.filter(schema::threshold::id.eq(query_threshold.id)))
        .execute(conn)
        .map_err(ApiError::from)?;

    Ok(JsonEmpty {})
}
