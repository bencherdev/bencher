use bencher_json::{JsonNewTestbed, JsonTestbed, ResourceId};
use bencher_rbac::project::Permission;
use diesel::{expression_methods::BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{pub_response_ok, response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::project::{
        testbed::{InsertTestbed, QueryTestbed},
        QueryProject,
    },
    model::user::auth::AuthUser,
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::into_json,
        resource_id::fn_resource_id,
    },
    ApiError,
};

use super::Resource;

const TESTBED_RESOURCE: Resource = Resource::Testbed;

#[derive(Deserialize, JsonSchema)]
pub struct DirPath {
    pub project: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/testbeds",
    tags = ["projects", "testbeds"]
}]
pub async fn dir_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<DirPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/testbeds",
    tags = ["projects", "testbeds"]
}]
pub async fn get_ls(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<DirPath>,
) -> Result<ResponseOk<Vec<JsonTestbed>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(TESTBED_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        path_params.into_inner(),
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
    path_params: DirPath,
    endpoint: Endpoint,
) -> Result<Vec<JsonTestbed>, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    Ok(schema::testbed::table
        .filter(schema::testbed::project_id.eq(query_project.id))
        .order((schema::testbed::name, schema::testbed::slug))
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
    rqctx: RequestContext<ApiContext>,
    path_params: Path<DirPath>,
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
    context: &ApiContext,
    path_params: DirPath,
    json_testbed: JsonNewTestbed,
    auth_user: &AuthUser,
) -> Result<JsonTestbed, ApiError> {
    let conn = &mut *context.conn().await;

    let insert_testbed = InsertTestbed::from_json(conn, &path_params.project, json_testbed)?;
    // Verify that the user is allowed
    QueryProject::is_allowed_id(
        conn,
        &context.rbac,
        insert_testbed.project_id,
        auth_user,
        Permission::Create,
    )?;

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
pub struct OnePath {
    pub project: ResourceId,
    pub testbed: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/testbeds/{testbed}",
    tags = ["projects", "testbeds"]
}]
pub async fn one_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OnePath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/testbeds/{testbed}",
    tags = ["projects", "testbeds"]
}]
pub async fn get_one(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OnePath>,
) -> Result<ResponseOk<JsonTestbed>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(TESTBED_RESOURCE, Method::GetOne);

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

fn_resource_id!(testbed);

async fn get_one_inner(
    context: &ApiContext,
    path_params: OnePath,
    auth_user: Option<&AuthUser>,
) -> Result<JsonTestbed, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

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
