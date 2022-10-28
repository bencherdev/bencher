use std::sync::Arc;

use bencher_json::{JsonNewTestbed, JsonTestbed, ResourceId};
use bencher_rbac::project::Permission;
use diesel::{expression_methods::BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    endpoints::{
        endpoint::{response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::project::{
        project::QueryProject,
        testbed::{InsertTestbed, QueryTestbed},
    },
    model::user::auth::AuthUser,
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::into_json,
        resource_id::fn_resource_id,
        Context,
    },
    ApiError,
};

use super::Resource;

const TESTBED_RESOURCE: Resource = Resource::Testbed;

#[derive(Deserialize, JsonSchema)]
pub struct GetDirParams {
    pub project: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/testbeds",
    tags = ["projects", "testbeds"]
}]
pub async fn dir_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetDirParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/testbeds",
    tags = ["projects", "testbeds"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetDirParams>,
) -> Result<ResponseOk<Vec<JsonTestbed>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(TESTBED_RESOURCE, Method::GetLs);

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
    path_params: GetDirParams,
    endpoint: Endpoint,
) -> Result<Vec<JsonTestbed>, ApiError> {
    let api_context = &mut *context.lock().await;
    let query_project = QueryProject::is_allowed_resource_id(
        api_context,
        &path_params.project,
        auth_user,
        Permission::View,
    )?;
    let conn = &mut api_context.database;

    Ok(schema::testbed::table
        .filter(schema::testbed::project_id.eq(query_project.id))
        .order(schema::testbed::name)
        .load::<QueryTestbed>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(into_json!(endpoint, conn))
        .collect())
}

#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/testbeds",
    tags = ["projects", "testbeds"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetDirParams>,
    body: TypedBody<JsonNewTestbed>,
) -> Result<ResponseAccepted<JsonTestbed>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(TESTBED_RESOURCE, Method::Post);

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
    path_params: GetDirParams,
    json_testbed: JsonNewTestbed,
    auth_user: &AuthUser,
) -> Result<JsonTestbed, ApiError> {
    let api_context = &mut *context.lock().await;
    let insert_testbed = InsertTestbed::from_json(
        &mut api_context.database,
        &path_params.project,
        json_testbed,
    )?;
    // Verify that the user is allowed
    QueryProject::is_allowed_id(
        api_context,
        insert_testbed.project_id,
        auth_user,
        Permission::Create,
    )?;
    let conn = &mut api_context.database;

    diesel::insert_into(schema::testbed::table)
        .values(&insert_testbed)
        .execute(conn)
        .map_err(api_error!())?;

    schema::testbed::table
        .filter(schema::testbed::uuid.eq(&insert_testbed.uuid))
        .first::<QueryTestbed>(conn)
        .map_err(api_error!())?
        .into_json(conn)
}

#[derive(Deserialize, JsonSchema)]
pub struct GetOneParams {
    pub project: ResourceId,
    pub testbed: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/testbeds/{testbed}",
    tags = ["projects", "testbeds"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetOneParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/testbeds/{testbed}",
    tags = ["projects", "testbeds"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetOneParams>,
) -> Result<ResponseOk<JsonTestbed>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(TESTBED_RESOURCE, Method::GetOne);

    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

fn_resource_id!(testbed);

async fn get_one_inner(
    context: &Context,
    path_params: GetOneParams,
    auth_user: &AuthUser,
) -> Result<JsonTestbed, ApiError> {
    let api_context = &mut *context.lock().await;
    let query_project = QueryProject::is_allowed_resource_id(
        api_context,
        &path_params.project,
        auth_user,
        Permission::View,
    )?;
    let conn = &mut api_context.database;

    schema::testbed::table
        .filter(
            schema::testbed::project_id
                .eq(query_project.id)
                .and(resource_id(&path_params.testbed)?),
        )
        .first::<QueryTestbed>(conn)
        .map_err(api_error!())?
        .into_json(conn)
}
