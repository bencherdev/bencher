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
        endpoint::{CorsResponse, Delete, Get, Post, Put, ResponseAccepted, ResponseOk},
        Endpoint,
    },
    error::{resource_conflict_err, resource_not_found_err},
    model::user::auth::{AuthUser, PubBearerToken},
    model::{
        project::{
            branch::QueryBranch,
            metric_kind::QueryMetricKind,
            testbed::QueryTestbed,
            threshold::{
                statistic::InsertStatistic, InsertThreshold, QueryThreshold, UpdateThreshold,
            },
            QueryProject,
        },
        user::auth::BearerToken,
    },
    schema,
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
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/thresholds",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_thresholds_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjThresholdsParams>,
    pagination_params: Query<ProjThresholdsPagination>,
) -> Result<ResponseOk<JsonThresholds>, HttpError> {
    let auth_user = AuthUser::from_pub_token(rqctx.context(), bearer_token).await?;
    let json = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        path_params.into_inner(),
        pagination_params.into_inner(),
    )
    .await?;
    Ok(Get::response_ok(json, auth_user.is_some()))
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: Option<&AuthUser>,
    path_params: ProjThresholdsParams,
    pagination_params: ProjThresholdsPagination,
) -> Result<JsonThresholds, HttpError> {
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

    let project = &query_project;
    Ok(query
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryThreshold>(conn)
        .map_err(resource_not_found_err!(Threshold, project))?
        .into_iter()
        .filter_map(|threshold| match threshold.into_json(conn) {
            Ok(threshold) => Some(threshold),
            Err(err) => {
                debug_assert!(false, "{err}");
                #[cfg(feature = "sentry")]
                sentry::capture_error(&err);
                None
            },
        })
        .collect())
}

#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/thresholds",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_threshold_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjThresholdsParams>,
    body: TypedBody<JsonNewThreshold>,
) -> Result<ResponseAccepted<JsonThreshold>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        &body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Post::auth_response_accepted(json))
}

async fn post_inner(
    context: &ApiContext,
    path_params: ProjThresholdsParams,
    json_threshold: &JsonNewThreshold,
    auth_user: &AuthUser,
) -> Result<JsonThreshold, HttpError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Create,
    )?;

    let project_id = query_project.id;
    // Verify that the branch, testbed, and metric kind are part of the same project
    let branch_id = QueryBranch::from_resource_id(conn, project_id, &json_threshold.branch)?.id;
    let testbed_id = QueryTestbed::from_resource_id(conn, project_id, &json_threshold.testbed)?.id;
    let metric_kind_id =
        QueryMetricKind::from_resource_id(conn, project_id, &json_threshold.metric_kind)?.id;

    // Create the new threshold
    let threshold_id = InsertThreshold::insert_from_json(
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
        .map_err(resource_not_found_err!(Threshold, threshold_id))?
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
    Ok(Endpoint::cors(&[Get.into(), Put.into(), Delete.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/thresholds/{threshold}",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_threshold_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjThresholdParams>,
) -> Result<ResponseOk<JsonThreshold>, HttpError> {
    let auth_user = AuthUser::from_pub_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(
        rqctx.context(),
        path_params.into_inner(),
        auth_user.as_ref(),
    )
    .await?;
    Ok(Get::response_ok(json, auth_user.is_some()))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjThresholdParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonThreshold, HttpError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    QueryThreshold::belonging_to(&query_project)
        .filter(schema::threshold::uuid.eq(path_params.threshold))
        .first::<QueryThreshold>(conn)
        .map_err(resource_not_found_err!(
            Threshold,
            (&query_project, path_params.threshold)
        ))?
        .into_json(conn)
}

#[endpoint {
    method = PUT,
    path =  "/v0/projects/{project}/thresholds/{threshold}",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_threshold_put(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjThresholdParams>,
    body: TypedBody<JsonUpdateThreshold>,
) -> Result<ResponseAccepted<JsonThreshold>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = put_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Put::auth_response_accepted(json))
}

async fn put_inner(
    context: &ApiContext,
    path_params: ProjThresholdParams,
    json_threshold: JsonUpdateThreshold,
    auth_user: &AuthUser,
) -> Result<JsonThreshold, HttpError> {
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
        .map_err(resource_not_found_err!(
            Threshold,
            (&query_project, path_params.threshold)
        ))?;

    // Insert the new statistic
    let insert_statistic = InsertStatistic::from_json(query_threshold.id, json_threshold.statistic);
    diesel::insert_into(schema::statistic::table)
        .values(&insert_statistic)
        .execute(conn)
        .map_err(resource_conflict_err!(
            Statistic,
            (&query_threshold, &insert_statistic)
        ))?;

    // Update the current threshold to use the new statistic
    diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(query_threshold.id)))
        .set(&UpdateThreshold::new_statistic(
            conn,
            insert_statistic.uuid,
        )?)
        .execute(conn)
        .map_err(resource_conflict_err!(
            Threshold,
            (&query_threshold, &insert_statistic)
        ))?;

    QueryThreshold::get(conn, query_threshold.id)?.into_json(conn)
}

#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/thresholds/{threshold}",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_threshold_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjThresholdParams>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = delete_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Delete::auth_response_accepted(json))
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjThresholdParams,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, HttpError> {
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
        .map_err(resource_not_found_err!(
            Threshold,
            (&query_project, path_params.threshold)
        ))?;
    diesel::delete(schema::threshold::table.filter(schema::threshold::id.eq(query_threshold.id)))
        .execute(conn)
        .map_err(resource_conflict_err!(Threshold, query_threshold))?;

    Ok(JsonEmpty {})
}
