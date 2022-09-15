use std::sync::Arc;

use bencher_json::{JsonNewToken, JsonToken, ResourceId};
use diesel::{expression_methods::BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    endpoints::{
        endpoint::{response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::{
        user::QueryUser,
        user::{
            auth::AuthUser,
            token::{InsertToken, QueryToken},
        },
    },
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        Context,
    },
    ApiError,
};

use super::Resource;

const TOKEN_RESOURCE: Resource = Resource::Token;

macro_rules! same_user {
    ($param:expr, $token:expr) => {
        if $param != $token {
            return Err(crate::error::ApiError::User(format!(
                "User IDs do not match for the path param ({}) and token ({})",
                $param, $token
            )));
        }
    };
}

#[derive(Deserialize, JsonSchema)]
pub struct GetLsParams {
    pub user: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/users/{user}/tokens",
    tags = ["users", "tokens"]
}]
pub async fn dir_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetLsParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/users/{user}/tokens",
    tags = ["users", "tokens"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetLsParams>,
) -> Result<ResponseOk<Vec<JsonToken>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(TOKEN_RESOURCE, Method::GetLs);

    let context = rqctx.context();
    let path_params = path_params.into_inner();
    let json = get_ls_inner(&auth_user, context, path_params)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_ls_inner(
    auth_user: &AuthUser,
    context: &Context,
    path_params: GetLsParams,
) -> Result<Vec<JsonToken>, ApiError> {
    let api_context = &mut *context.lock().await;
    let conn = &mut api_context.db_conn;
    let query_user = QueryUser::from_resource_id(conn, &path_params.user)?;

    same_user!(query_user.id, auth_user.id);

    let json: Vec<JsonToken> = schema::token::table
        .filter(schema::token::user_id.eq(query_user.id))
        .order((schema::token::creation, schema::token::expiration))
        .load::<QueryToken>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(|query| query.into_json(conn).ok())
        .collect();

    Ok(json)
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/tokens",
    tags = ["tokens"]
}]
pub async fn post_options(_rqctx: Arc<RequestContext<Context>>) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path = "/v0/tokens",
    tags = ["tokens"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonNewToken>,
) -> Result<ResponseAccepted<JsonToken>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(TOKEN_RESOURCE, Method::Post);

    let context = rqctx.context();
    let json_token = body.into_inner();
    let json = post_inner(&auth_user, context, json_token)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    auth_user: &AuthUser,
    context: &Context,
    json_token: JsonNewToken,
) -> Result<JsonToken, ApiError> {
    let api_context = &mut *context.lock().await;
    let conn = &mut api_context.db_conn;

    let insert_token =
        InsertToken::from_json(conn, json_token, auth_user.id, &api_context.secret_key)?;
    diesel::insert_into(schema::token::table)
        .values(&insert_token)
        .execute(conn)
        .map_err(api_error!())?;

    schema::token::table
        .filter(schema::token::uuid.eq(&insert_token.uuid))
        .first::<QueryToken>(conn)
        .map_err(api_error!())?
        .into_json(conn)
        .map_err(api_error!())
}

#[derive(Deserialize, JsonSchema)]
pub struct GetOneParams {
    pub user: ResourceId,
    pub token: Uuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/users/{user}/tokens/{token}",
    tags = ["users", "tokens"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetOneParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/users/{user}/tokens/{token}",
    tags = ["users", "tokens"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetOneParams>,
) -> Result<ResponseOk<JsonToken>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(TOKEN_RESOURCE, Method::GetOne);

    let context = rqctx.context();
    let path_params = path_params.into_inner();
    let json = get_one_inner(&auth_user, context, path_params)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_one_inner(
    auth_user: &AuthUser,
    context: &Context,
    path_params: GetOneParams,
) -> Result<JsonToken, ApiError> {
    let api_context = &mut *context.lock().await;
    let conn = &mut api_context.db_conn;
    let query_user = QueryUser::from_resource_id(conn, &path_params.user)?;

    same_user!(query_user.id, auth_user.id);

    schema::token::table
        .filter(
            schema::token::user_id
                .eq(query_user.id)
                .and(schema::token::uuid.eq(&path_params.token.to_string())),
        )
        .first::<QueryToken>(conn)
        .map_err(api_error!())?
        .into_json(conn)
        .map_err(api_error!())
}
