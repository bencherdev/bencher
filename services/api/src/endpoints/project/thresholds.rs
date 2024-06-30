use bencher_json::{
    project::threshold::{
        JsonNewThreshold, JsonThreshold, JsonThresholdQuery, JsonThresholdQueryParams,
        JsonUpdateThreshold,
    },
    JsonDirection, JsonPagination, JsonThresholds, ModelUuid, ResourceId, ThresholdUuid,
};
use bencher_rbac::project::Permission;
use diesel::{BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    conn_lock,
    context::ApiContext,
    endpoints::{
        endpoint::{
            CorsLsResponse, CorsResponse, Delete, Get, Post, Put, ResponseCreated, ResponseDeleted,
            ResponseOk, ResponseOkLs,
        },
        Endpoint,
    },
    error::{
        bad_request_error, resource_conflict_err, resource_not_found_err, resource_not_found_error,
        BencherResource,
    },
    model::{
        project::{
            branch::QueryBranch,
            measure::QueryMeasure,
            testbed::QueryTestbed,
            threshold::{model::QueryModel, InsertThreshold, QueryThreshold},
            QueryProject,
        },
        user::auth::{AuthUser, BearerToken, PubBearerToken},
    },
    schema,
    util::{
        headers::TotalCount,
        name_id::{filter_branch_name_id, filter_measure_name_id, filter_testbed_name_id},
    },
};

#[derive(Deserialize, JsonSchema)]
pub struct ProjThresholdsParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
}

pub type ProjThresholdsPagination = JsonPagination<ProjThresholdsSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjThresholdsSort {
    /// Sort by threshold creation date time.
    #[default]
    Created,
    /// Sort by threshold modified date time.
    Modified,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/thresholds",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_thresholds_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjThresholdsParams>,
    _pagination_params: Query<ProjThresholdsPagination>,
    _query_params: Query<JsonThresholdQueryParams>,
) -> Result<CorsLsResponse, HttpError> {
    Ok(Endpoint::cors_ls(&[Get.into(), Post.into()]))
}

/// List thresholds for a project
///
/// List all thresholds for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
/// By default, the thresholds are sorted by creation date time in chronological order.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/thresholds",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_thresholds_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjThresholdsParams>,
    pagination_params: Query<ProjThresholdsPagination>,
    query_params: Query<JsonThresholdQueryParams>,
) -> Result<ResponseOkLs<JsonThresholds>, HttpError> {
    // Second round of marshaling
    let json_threshold_query = query_params
        .into_inner()
        .try_into()
        .map_err(bad_request_error)?;

    let auth_user = AuthUser::new_pub(&rqctx).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        json_threshold_query,
    )
    .await?;
    Ok(Get::response_ok_ls(json, auth_user.is_some(), total_count))
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: Option<&AuthUser>,
    path_params: ProjThresholdsParams,
    pagination_params: ProjThresholdsPagination,
    query_params: JsonThresholdQuery,
) -> Result<(JsonThresholds, TotalCount), HttpError> {
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
    )?;

    // Separate out this query to prevent a deadlock when getting the conn_lock
    let thresholds = get_ls_query(&query_project, &pagination_params, &query_params)?
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryThreshold>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Threshold,
            (&query_project, &pagination_params, &query_params)
        ))?;

    let mut json_thresholds = Vec::with_capacity(thresholds.len());
    for threshold in thresholds {
        match threshold.into_json(conn_lock!(context)) {
            Ok(threshold) => json_thresholds.push(threshold),
            Err(err) => {
                debug_assert!(false, "{err}");
                #[cfg(feature = "sentry")]
                sentry::capture_error(&err);
            },
        }
    }

    let total_count = get_ls_query(&query_project, &pagination_params, &query_params)?
        .count()
        .get_result::<i64>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Threshold,
            (&query_project, &pagination_params, &query_params)
        ))?
        .try_into()?;

    Ok((json_thresholds.into(), total_count))
}

fn get_ls_query<'q>(
    query_project: &'q QueryProject,
    pagination_params: &ProjThresholdsPagination,
    query_params: &'q JsonThresholdQuery,
) -> Result<BoxedQuery<'q>, HttpError> {
    let mut query = QueryThreshold::belonging_to(query_project)
        .inner_join(schema::branch::table)
        .inner_join(schema::testbed::table)
        .inner_join(schema::measure::table)
        .into_boxed();

    if let Some(branch) = query_params.branch.as_ref() {
        filter_branch_name_id!(query, branch);
    }
    if let Some(testbed) = query_params.testbed.as_ref() {
        filter_testbed_name_id!(query, testbed);
    }
    if let Some(measure) = query_params.measure.as_ref() {
        filter_measure_name_id!(query, measure);
    }

    Ok(match pagination_params.order() {
        ProjThresholdsSort::Created => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::threshold::created.asc()),
            Some(JsonDirection::Desc) => query.order(schema::threshold::created.desc()),
        },
        ProjThresholdsSort::Modified => match pagination_params.direction {
            Some(JsonDirection::Asc) => query.order(schema::threshold::modified.asc()),
            Some(JsonDirection::Desc) | None => query.order(schema::threshold::modified.desc()),
        },
    }
    .select(QueryThreshold::as_select()))
}

// TODO refactor out internal types
type BoxedQuery<'q> = diesel::internal::table_macro::BoxedSelectStatement<
    'q,
    diesel::helper_types::AsSelect<QueryThreshold, diesel::sqlite::Sqlite>,
    diesel::internal::table_macro::FromClause<
        diesel::helper_types::InnerJoinQuerySource<
            diesel::helper_types::InnerJoinQuerySource<
                diesel::helper_types::InnerJoinQuerySource<
                    schema::threshold::table,
                    schema::branch::table,
                >,
                schema::testbed::table,
            >,
            schema::measure::table,
        >,
    >,
    diesel::sqlite::Sqlite,
>;

/// Create a threshold
///
/// Create a threshold for a project.
/// The user must have `create` permissions for the project.
/// There can only be one threshold for any unique combination of: branch, testbed, and measure.
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
) -> Result<ResponseCreated<JsonThreshold>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        &body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    context: &ApiContext,
    path_params: ProjThresholdsParams,
    json_threshold: &JsonNewThreshold,
    auth_user: &AuthUser,
) -> Result<JsonThreshold, HttpError> {
    // Validate the new model
    json_threshold.model.validate().map_err(bad_request_error)?;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Create,
    )?;

    let project_id = query_project.id;
    // Verify that the branch, testbed, and measure are part of the same project
    let branch_id =
        QueryBranch::from_name_id(conn_lock!(context), project_id, &json_threshold.branch)?.id;
    let testbed_id =
        QueryTestbed::from_name_id(conn_lock!(context), project_id, &json_threshold.testbed)?.id;
    let measure_id =
        QueryMeasure::from_name_id(conn_lock!(context), project_id, &json_threshold.measure)?.id;

    // Create the new threshold
    let threshold_id = InsertThreshold::insert_from_json(
        conn_lock!(context),
        project_id,
        branch_id,
        testbed_id,
        measure_id,
        json_threshold.model,
    )?;

    // Return the new threshold with the new model
    conn_lock!(context, |conn| schema::threshold::table
        .filter(schema::threshold::id.eq(threshold_id))
        .first::<QueryThreshold>(conn)
        .map_err(resource_not_found_err!(Threshold, threshold_id))?
        .into_json(conn))
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjThresholdParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
    /// The UUID for a threshold.
    pub threshold: ThresholdUuid,
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjThresholdQuery {
    /// View the threshold with the specified model UUID.
    /// This can be useful for viewing thresholds with historical models
    /// that have since been replaced by a new model.
    /// If not specified, then the current model is used.
    pub model: Option<ModelUuid>,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/thresholds/{threshold}",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_threshold_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjThresholdParams>,
    _query_params: Query<ProjThresholdQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Put.into(), Delete.into()]))
}

/// View a threshold
///
/// View a threshold for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/thresholds/{threshold}",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_threshold_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjThresholdParams>,
    query_params: Query<ProjThresholdQuery>,
) -> Result<ResponseOk<JsonThreshold>, HttpError> {
    let auth_user = AuthUser::from_pub_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(
        rqctx.context(),
        path_params.into_inner(),
        query_params.into_inner(),
        auth_user.as_ref(),
    )
    .await?;
    Ok(Get::response_ok(json, auth_user.is_some()))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjThresholdParams,
    query_params: ProjThresholdQuery,
    auth_user: Option<&AuthUser>,
) -> Result<JsonThreshold, HttpError> {
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
    )?;

    let query_threshold =
        QueryThreshold::get_with_uuid(conn_lock!(context), &query_project, path_params.threshold)?;

    if let Some(model_uuid) = query_params.model {
        let query_model = QueryModel::from_uuid(conn_lock!(context), query_project.id, model_uuid)?;
        if query_model.threshold_id != query_threshold.id {
            return Err(resource_not_found_error(
                BencherResource::Model,
                model_uuid,
                format!(
                    "Specified model {model_uuid} does not belong to threshold {threshold_uuid}",
                    threshold_uuid = query_threshold.uuid
                ),
            ));
        }
        query_threshold.into_json_for_model(conn_lock!(context), query_model)
    } else {
        query_threshold.into_json(conn_lock!(context))
    }
}

/// Update a threshold
///
/// Update a threshold for a project.
/// The user must have `edit` permissions for the project.
/// The new model will be added to the threshold and used going forward.
/// The old model will be replaced but still show up in the report history and alerts created when it was active.
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
) -> Result<ResponseOk<JsonThreshold>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = put_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Put::auth_response_ok(json))
}

async fn put_inner(
    context: &ApiContext,
    path_params: ProjThresholdParams,
    json_threshold: JsonUpdateThreshold,
    auth_user: &AuthUser,
) -> Result<JsonThreshold, HttpError> {
    // Validate the new model
    json_threshold.model.validate().map_err(bad_request_error)?;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Edit,
    )?;

    // Get the current threshold
    let query_threshold =
        QueryThreshold::get_with_uuid(conn_lock!(context), &query_project, path_params.threshold)?;

    // Update the current threshold with the new model
    // Hold the database lock across the entire `update_from_json` call
    query_threshold.update_from_json(conn_lock!(context), json_threshold.model)?;

    conn_lock!(context, |conn| QueryThreshold::get(
        conn,
        query_threshold.id
    )?
    .into_json(conn))
}

/// Delete a threshold
///
/// Delete a threshold for a project.
/// The user must have `delete` permissions for the project.
/// A thresholds must be deleted before its branch, testbed, or measure can be deleted.
#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/thresholds/{threshold}",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_threshold_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjThresholdParams>,
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjThresholdParams,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Delete,
    )?;

    let query_threshold =
        QueryThreshold::get_with_uuid(conn_lock!(context), &query_project, path_params.threshold)?;

    diesel::delete(schema::threshold::table.filter(schema::threshold::id.eq(query_threshold.id)))
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Threshold, query_threshold))?;

    Ok(())
}
