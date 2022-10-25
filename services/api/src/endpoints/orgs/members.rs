use std::sync::Arc;

use bencher_json::{member::JsonUpdateMember, JsonMember, ResourceId};
use bencher_rbac::organization::Permission;
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl, SqliteConnection};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    endpoints::{
        endpoint::{response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::user::{auth::AuthUser, QueryUser},
    model::{organization::QueryOrganization, user::member::QueryMember},
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::into_json,
        Context,
    },
    ApiError,
};

use super::Resource;

const MEMBER_RESOURCE: Resource = Resource::Member;

#[derive(Deserialize, JsonSchema)]
pub struct GetLsParams {
    pub organization: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/members",
    tags = ["organizations", "members"]
}]
pub async fn dir_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetLsParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/members",
    tags = ["organizations", "members"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetLsParams>,
) -> Result<ResponseOk<Vec<JsonMember>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(MEMBER_RESOURCE, Method::GetLs);

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
    path_params: GetLsParams,
    endpoint: Endpoint,
) -> Result<Vec<JsonMember>, ApiError> {
    let api_context = &mut *context.lock().await;
    let query_organization = QueryOrganization::is_allowed_resource_id(
        api_context,
        &path_params.organization,
        auth_user,
        Permission::View,
    )?;
    let conn = &mut api_context.database;

    Ok(schema::user::table
        .inner_join(
            schema::organization_role::table
                .on(schema::user::id.eq(schema::organization_role::user_id)),
        )
        .filter(schema::organization_role::organization_id.eq(query_organization.id))
        .select((
            schema::user::uuid,
            schema::user::name,
            schema::user::slug,
            schema::user::email,
            schema::organization_role::role,
        ))
        .order(schema::user::email)
        .load::<QueryMember>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(into_json!(endpoint))
        .collect())
}

#[derive(Deserialize, JsonSchema)]
pub struct GetOneParams {
    pub organization: ResourceId,
    pub user: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/members/{user}",
    tags = ["organizations", "members"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetOneParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/members/{user}",
    tags = ["organizations", "members"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetOneParams>,
) -> Result<ResponseOk<JsonMember>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(MEMBER_RESOURCE, Method::GetOne);

    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_one_inner(
    context: &Context,
    path_params: GetOneParams,
    auth_user: &AuthUser,
) -> Result<JsonMember, ApiError> {
    let api_context = &mut *context.lock().await;
    let query_organization = QueryOrganization::is_allowed_resource_id(
        api_context,
        &path_params.organization,
        auth_user,
        Permission::View,
    )?;
    let conn = &mut api_context.database;
    let query_user = QueryUser::from_resource_id(conn, &path_params.user)?;

    json_member(conn, query_user.id, query_organization.id)
}

#[endpoint {
    method = PUT,
    path =  "/v0/organizations/{organization}/members/{user}",
    tags = ["organizations", "members"]
}]
pub async fn put(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetOneParams>,
    body: TypedBody<JsonUpdateMember>,
) -> Result<ResponseAccepted<JsonMember>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(MEMBER_RESOURCE, Method::Post);

    let json = put_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn put_inner(
    context: &Context,
    path_params: GetOneParams,
    json_update: JsonUpdateMember,
    auth_user: &AuthUser,
) -> Result<JsonMember, ApiError> {
    let api_context = &mut *context.lock().await;

    let query_user = QueryUser::from_resource_id(&mut api_context.database, &json_update.user)?;
    let query_organization =
        QueryOrganization::from_resource_id(&mut api_context.database, &path_params.organization)?;

    if let Some(role) = json_update.role {
        // Verify that the user is allowed to update member role
        QueryOrganization::is_allowed_id(
            api_context,
            query_organization.id,
            auth_user,
            Permission::EditRole,
        )?;
        diesel::update(
            schema::organization_role::table
                .filter(schema::organization_role::user_id.eq(query_user.id))
                .filter(schema::organization_role::organization_id.eq(query_organization.id)),
        )
        .set(schema::organization_role::role.eq(role.to_string()))
        .execute(&mut api_context.database)
        .map_err(api_error!())?;
    }

    json_member(
        &mut api_context.database,
        query_user.id,
        query_organization.id,
    )
}

fn json_member(
    conn: &mut SqliteConnection,
    user_id: i32,
    organization_id: i32,
) -> Result<JsonMember, ApiError> {
    schema::user::table
        .inner_join(
            schema::organization_role::table
                .on(schema::user::id.eq(schema::organization_role::user_id)),
        )
        .filter(schema::organization_role::user_id.eq(user_id))
        .filter(schema::organization_role::organization_id.eq(organization_id))
        .select((
            schema::user::uuid,
            schema::user::name,
            schema::user::slug,
            schema::user::email,
            schema::organization_role::role,
        ))
        .order(schema::user::email)
        .first::<QueryMember>(conn)
        .map_err(api_error!())?
        .into_json()
}
