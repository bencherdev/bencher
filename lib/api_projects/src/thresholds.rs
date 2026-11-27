use bencher_endpoint::{
    CorsResponse, Delete, Endpoint, Get, Post, Put, ResponseCreated, ResponseDeleted, ResponseOk,
    TotalCount,
};
use bencher_json::{
    JsonDirection, JsonPagination, JsonThresholds, Model, ModelSlot, ModelUuid, ProjectResourceId,
    ThresholdUuid,
    project::threshold::{
        JsonNewThreshold, JsonThreshold, JsonThresholdQuery, JsonThresholdQueryParams,
        JsonUpdateThreshold,
    },
};
use bencher_rbac::project::Permission;
use bencher_schema::{
    actor_conn, auth_conn,
    context::ApiContext,
    error::{
        BencherResource, bad_request_error, resource_conflict_err, resource_not_found_err,
        resource_not_found_error, with_auth_hint, with_token_hint,
    },
    model::{
        project::{
            QueryProject,
            branch::QueryBranch,
            measure::QueryMeasure,
            testbed::QueryTestbed,
            threshold::{
                InsertThreshold, QueryThreshold, ThresholdDimensions, ThresholdSlots,
                ThresholdSpec, model::QueryModel,
            },
        },
        user::{
            actor::{ApiActor, PubProjectBearerToken},
            auth::{AuthUser, BearerToken},
        },
    },
    schema, write_conn,
};
use diesel::{
    BelongingToDsl as _, BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _,
    RunQueryDsl as _, SelectableHelper as _,
};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use futures::{StreamExt as _, stream::FuturesOrdered};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::macros::{filter_branch_name_id, filter_measure_name_id, filter_testbed_name_id};

#[derive(Deserialize, JsonSchema)]
pub struct ProjThresholdsParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
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
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// List thresholds for a project
///
/// List all thresholds for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project,
/// or provide a valid project key for the project.
/// By default, the thresholds are sorted by creation date time in chronological order.
/// The HTTP response header `X-Total-Count` contains the total number of thresholds.
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
) -> Result<ResponseOk<JsonThresholds>, HttpError> {
    // Second round of marshaling
    let json_threshold_query = query_params
        .into_inner()
        .try_into()
        .map_err(bad_request_error)?;

    let api_actor = ApiActor::new(&rqctx).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        json_threshold_query,
        &api_actor,
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
    path_params: ProjThresholdsParams,
    pagination_params: ProjThresholdsPagination,
    query_params: JsonThresholdQuery,
    api_actor: &ApiActor,
) -> Result<(JsonThresholds, TotalCount), HttpError> {
    let query_project = QueryProject::is_allowed_actor_pub(
        actor_conn!(context, api_actor),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        api_actor,
    )?;

    let thresholds = get_ls_query(&query_project, &pagination_params, &query_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryThreshold>(actor_conn!(context, api_actor))
        .map_err(resource_not_found_err!(
            Threshold,
            (&query_project, &pagination_params, &query_params)
        ))?;

    // Drop connection lock before iterating
    let json_thresholds = thresholds
        .into_iter()
        .map(|threshold| async { threshold.into_json(actor_conn!(context, api_actor)) })
        .collect::<FuturesOrdered<_>>()
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .filter_map(|threshold| match threshold {
            Ok(threshold) => Some(threshold),
            Err(err) => {
                debug_assert!(false, "{err}");
                #[cfg(feature = "sentry")]
                sentry::capture_error(&err);
                None
            },
        })
        .collect::<Vec<_>>();

    let total_count = get_ls_query(&query_project, &pagination_params, &query_params)
        .count()
        .get_result::<i64>(actor_conn!(context, api_actor))
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
) -> BoxedQuery<'q> {
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

    if let Some(true) = query_params.archived {
        query = query.filter(
            schema::branch::archived
                .is_not_null()
                .or(schema::testbed::archived.is_not_null())
                .or(schema::measure::archived.is_not_null()),
        );
    } else {
        query = query.filter(
            schema::branch::archived
                .is_null()
                .and(schema::testbed::archived.is_null())
                .and(schema::measure::archived.is_null()),
        );
    }

    match pagination_params.order() {
        ProjThresholdsSort::Created => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::threshold::created.asc()),
            Some(JsonDirection::Desc) => query.order(schema::threshold::created.desc()),
        },
        ProjThresholdsSort::Modified => match pagination_params.direction {
            Some(JsonDirection::Asc) => query.order(schema::threshold::modified.asc()),
            Some(JsonDirection::Desc) | None => query.order(schema::threshold::modified.desc()),
        },
    }
    .select(QueryThreshold::as_select())
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
/// The user must have `create` permissions for the project,
/// or provide a valid project key for the project.
/// There can only be one threshold for any unique combination of:
/// branch, testbed, parameters filter, measure, and metric name.
/// A threshold with no parameters filter checks every variant,
/// and a threshold that names no metric checks the conventional `value` name.
/// Every threshold that matches a metric row runs: there is no winner among them.
/// A branch, testbed, and measure can have at most 64 thresholds with a model in either slot.
/// The model's test must check a metric, and the changepoint model's test must check a line.
#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/thresholds",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_threshold_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjThresholdsParams>,
    body: TypedBody<JsonNewThreshold>,
) -> Result<ResponseCreated<JsonThreshold>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        &body.into_inner(),
        &api_actor,
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Post::auth_response_created(json))
}

pub async fn post_inner(
    context: &ApiContext,
    path_params: ProjThresholdsParams,
    json_threshold: &JsonNewThreshold,
    api_actor: &ApiActor,
) -> Result<JsonThreshold, HttpError> {
    // Validate the new models
    let models = ThresholdSlots {
        model: Some(json_threshold.model),
        changepoint_model: json_threshold.changepoint_model,
    };
    for slot in ModelSlot::ALL {
        if let Some(model) = models.get(slot) {
            validate_slot_model(slot, model)?;
        }
    }

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed_actor_auth(
        auth_conn!(context),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        api_actor,
        Permission::Create,
    )?;

    let project_id = query_project.id;
    // Verify that the branch, testbed, and measure are part of the same project
    let branch_id =
        QueryBranch::from_name_id(auth_conn!(context), project_id, &json_threshold.branch)?.id;
    let testbed_id =
        QueryTestbed::from_name_id(auth_conn!(context), project_id, &json_threshold.testbed)?.id;
    let measure_id =
        QueryMeasure::from_name_id(auth_conn!(context), project_id, &json_threshold.measure)?.id;

    let dimensions = ThresholdDimensions::new(
        branch_id,
        testbed_id,
        json_threshold.parameters.clone(),
        measure_id,
        json_threshold.metric.clone(),
    );
    let threshold_id =
        InsertThreshold::from_models(context, project_id, dimensions, models).await?;

    // Get the new threshold with the new model
    auth_conn!(context, |conn| {
        schema::threshold::table
            .filter(schema::threshold::id.eq(threshold_id))
            .first::<QueryThreshold>(conn)
            .map_err(resource_not_found_err!(Threshold, threshold_id))
            .map(|threshold| threshold.into_json(conn))?
    })
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjThresholdParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
    /// The UUID for a threshold.
    pub threshold: ThresholdUuid,
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjThresholdQuery {
    /// View the threshold with the specified model UUID.
    /// This can be used to view a threshold with a historical model
    /// that has since been replaced by a new model.
    /// If not specified, then the current model is used.
    pub model: Option<ModelUuid>,
    /// View the threshold with the specified changepoint model UUID.
    /// This can be used to view a threshold with a historical changepoint model
    /// that has since been replaced by a new changepoint model.
    /// If not specified, then the current changepoint model is used.
    pub changepoint_model: Option<ModelUuid>,
}

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
/// If the project is private, then the user must be authenticated and have `view` permissions for the project,
/// or provide a valid project key for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/thresholds/{threshold}",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_threshold_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjThresholdParams>,
    query_params: Query<ProjThresholdQuery>,
) -> Result<ResponseOk<JsonThreshold>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = get_one_inner(
        rqctx.context(),
        path_params.into_inner(),
        query_params.into_inner(),
        &api_actor,
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Get::response_ok(json, api_actor.is_auth()))
}

pub async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjThresholdParams,
    query_params: ProjThresholdQuery,
    api_actor: &ApiActor,
) -> Result<JsonThreshold, HttpError> {
    actor_conn!(context, api_actor, |conn| {
        let query_project = QueryProject::is_allowed_actor_pub(
            conn,
            &context.rbac,
            #[cfg(feature = "plus")]
            &context.rate_limiting,
            &path_params.project,
            api_actor,
        )?;

        let query_threshold =
            QueryThreshold::get_with_uuid(conn, &query_project, path_params.threshold)?;

        let ProjThresholdQuery {
            model,
            changepoint_model,
        } = query_params;
        let mut pinned = ThresholdSlots::default();
        for (slot, model_uuid) in ModelSlot::ALL.into_iter().zip([model, changepoint_model]) {
            let Some(model_uuid) = model_uuid else {
                continue;
            };
            let query_model = QueryModel::from_uuid(conn, query_project.id, model_uuid)?;
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
            slot.admit(query_model.test).map_err(bad_request_error)?;
            pinned.set(slot, query_model);
        }
        query_threshold.into_json_for_models(conn, pinned, None, ThresholdSpec::Testbed)
    })
}

/// Update a threshold
///
/// Update a threshold for a project.
/// The user must have `edit` permissions for the project,
/// or provide a valid project key for the project.
/// The new model will be added to the threshold and used going forward.
/// The old model will be replaced but still show up in the report history and alerts created when it was active.
/// A threshold with neither model can only be given one while its branch, testbed, and measure have fewer than 64 thresholds with a model in either slot.
/// The changepoint model changes only when the update names it, and its test must check a line.
/// An update changes every slot it names, or none of them.
#[endpoint {
    method = PUT,
    path =  "/v0/projects/{project}/thresholds/{threshold}",
    tags = ["projects", "thresholds"]
}]
pub async fn proj_threshold_put(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjThresholdParams>,
    body: TypedBody<JsonUpdateThreshold>,
) -> Result<ResponseOk<JsonThreshold>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = put_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &api_actor,
    )
    .await
    .map_err(with_auth_hint)?;
    Ok(Put::auth_response_ok(json))
}

pub async fn put_inner(
    context: &ApiContext,
    path_params: ProjThresholdParams,
    json_threshold: JsonUpdateThreshold,
    api_actor: &ApiActor,
) -> Result<JsonThreshold, HttpError> {
    // Validate the new models
    let (model, changepoint_model) = json_threshold.into_models();
    if let Some(model) = model {
        validate_slot_model(ModelSlot::Model, model)?;
    }
    if let Some(Some(changepoint_model)) = changepoint_model {
        validate_slot_model(ModelSlot::ChangepointModel, changepoint_model)?;
    }

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed_actor_auth(
        auth_conn!(context),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        api_actor,
        Permission::Edit,
    )?;

    // Get the current threshold
    let query_threshold =
        QueryThreshold::get_with_uuid(auth_conn!(context), &query_project, path_params.threshold)?;

    // Update the current threshold with the new models, if changed
    query_threshold
        .update_slots_if_changed(
            context,
            ThresholdSlots {
                model: Some(model),
                changepoint_model,
            },
        )
        .await?;

    // Get the updated threshold with the new model
    auth_conn!(context, |conn| {
        QueryThreshold::get(conn, query_threshold.id).map(|threshold| threshold.into_json(conn))?
    })
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
    delete_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(with_token_hint)?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjThresholdParams,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        auth_conn!(context),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        auth_user,
        Permission::Delete,
    )?;

    let query_threshold =
        QueryThreshold::get_with_uuid(auth_conn!(context), &query_project, path_params.threshold)?;

    diesel::delete(schema::threshold::table.filter(schema::threshold::id.eq(query_threshold.id)))
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(Threshold, query_threshold))?;

    Ok(())
}

/// Refuse a model that belongs in the other slot, then a model its own test does not allow.
fn validate_slot_model(slot: ModelSlot, model: Model) -> Result<(), HttpError> {
    slot.admit(model.test).map_err(bad_request_error)?;
    model.validate().map_err(bad_request_error)
}
