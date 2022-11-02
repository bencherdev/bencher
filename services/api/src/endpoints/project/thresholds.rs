use std::sync::Arc;

use bencher_json::{
    project::threshold::{JsonNewThreshold, JsonThreshold},
    ResourceId,
};
use bencher_rbac::project::Permission;
use diesel::{
    expression_methods::BoolExpressionMethods, ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl,
};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    context::Context,
    endpoints::{
        endpoint::{response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::project::{
        threshold::{InsertThreshold, QueryThreshold},
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

use super::Resource;

const THRESHOLD_RESOURCE: Resource = Resource::Threshold;

#[derive(Deserialize, JsonSchema)]
pub struct DirPath {
    pub project: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/thresholds",
    tags = ["projects", "thresholds"]
}]
pub async fn dir_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<DirPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/thresholds",
    tags = ["projects", "thresholds"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<DirPath>,
) -> Result<ResponseOk<Vec<JsonThreshold>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(THRESHOLD_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        &auth_user,
        path_params.into_inner(),
        endpoint,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_ls_inner(
    context: &Context,
    auth_user: &AuthUser,
    path_params: DirPath,
    endpoint: Endpoint,
) -> Result<Vec<JsonThreshold>, ApiError> {
    let api_context = &mut *context.lock().await;
    let query_project = QueryProject::is_allowed_resource_id(
        api_context,
        &path_params.project,
        auth_user,
        Permission::View,
    )?;
    let conn = &mut api_context.database;

    Ok(schema::threshold::table
        .left_join(schema::testbed::table.on(schema::threshold::testbed_id.eq(schema::testbed::id)))
        .filter(schema::testbed::project_id.eq(query_project.id))
        .order(schema::threshold::id)
        .select((
            schema::threshold::id,
            schema::threshold::uuid,
            schema::threshold::branch_id,
            schema::threshold::testbed_id,
            schema::threshold::kind,
            schema::threshold::statistic_id,
        ))
        .order(schema::threshold::id)
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
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<DirPath>,
    body: TypedBody<JsonNewThreshold>,
) -> Result<ResponseAccepted<JsonThreshold>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(THRESHOLD_RESOURCE, Method::Post);

    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &Context,
    path_params: DirPath,
    json_threshold: JsonNewThreshold,
    auth_user: &AuthUser,
) -> Result<JsonThreshold, ApiError> {
    let api_context = &mut *context.lock().await;

    // Verify that the branch and testbed are part of the same project
    let SameProject {
        project_id,
        branch_id,
        testbed_id,
    } = SameProject::validate(
        &mut api_context.database,
        &path_params.project,
        json_threshold.branch,
        json_threshold.testbed,
    )?;

    // Verify that the user is allowed
    QueryProject::is_allowed_id(api_context, project_id, auth_user, Permission::Create)?;
    let conn = &mut api_context.database;

    let insert_threshold = InsertThreshold::from_json(conn, branch_id, testbed_id, json_threshold)?;
    diesel::insert_into(schema::threshold::table)
        .values(&insert_threshold)
        .execute(conn)
        .map_err(api_error!())?;

    schema::threshold::table
        .filter(schema::threshold::uuid.eq(&insert_threshold.uuid))
        .first::<QueryThreshold>(conn)
        .map_err(api_error!())?
        .into_json(conn)
}

#[derive(Deserialize, JsonSchema)]
pub struct OnePath {
    pub project: ResourceId,
    pub threshold: Uuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/thresholds/{threshold}",
    tags = ["projects", "thresholds"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<OnePath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/thresholds/{threshold}",
    tags = ["projects", "thresholds"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<OnePath>,
) -> Result<ResponseOk<JsonThreshold>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(THRESHOLD_RESOURCE, Method::GetOne);

    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_one_inner(
    context: &Context,
    path_params: OnePath,
    auth_user: &AuthUser,
) -> Result<JsonThreshold, ApiError> {
    let api_context = &mut *context.lock().await;
    let query_project = QueryProject::is_allowed_resource_id(
        api_context,
        &path_params.project,
        auth_user,
        Permission::View,
    )?;
    let conn = &mut api_context.database;

    schema::threshold::table
        .left_join(schema::testbed::table.on(schema::threshold::testbed_id.eq(schema::testbed::id)))
        .filter(
            schema::testbed::project_id
                .eq(query_project.id)
                .and(schema::threshold::uuid.eq(path_params.threshold.to_string())),
        )
        .select((
            schema::threshold::id,
            schema::threshold::uuid,
            schema::threshold::branch_id,
            schema::threshold::testbed_id,
            schema::threshold::kind,
            schema::threshold::statistic_id,
        ))
        .first::<QueryThreshold>(conn)
        .map_err(api_error!())?
        .into_json(conn)
}
