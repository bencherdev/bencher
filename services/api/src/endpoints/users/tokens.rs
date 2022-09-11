use std::sync::Arc;

use bencher_json::{JsonNewToken, JsonToken, ResourceId};
use diesel::{expression_methods::BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{
    endpoint, HttpError, HttpResponseAccepted, HttpResponseHeaders, HttpResponseOk, Path,
    RequestContext, TypedBody,
};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    endpoints::{
        endpoint::{response_accepted, response_ok},
        Endpoint, Method,
    },
    model::{
        user::token::{InsertToken, QueryToken},
        user::QueryUser,
    },
    schema,
    util::{cors::get_cors, headers::CorsHeaders, http_error, map_http_error, Context},
    ApiError,
};

use super::Resource;

const TOKEN_RESOURCE: Resource = Resource::Token;

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
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
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
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<JsonToken>>, CorsHeaders>, HttpError> {
    let user_id = QueryUser::auth(&rqctx).await?;
    let endpoint = Endpoint::new(TOKEN_RESOURCE, Method::GetLs);

    let context = rqctx.context();
    let path_params = path_params.into_inner();
    let json = get_ls_inner(user_id, context, path_params)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

pub async fn get_ls_inner(
    user_id: i32,
    context: &Context,
    path_params: GetLsParams,
) -> Result<Vec<JsonToken>, ApiError> {
    let context = &mut *context.lock().await;
    let conn = &mut context.db;
    let query_user = QueryUser::from_resource_id(conn, &path_params.user)?;

    // TODO make smarter once permissions are a thing
    if query_user.id != user_id {
        return Err(http_error!("Failed to get token.").into());
    }

    let json: Vec<JsonToken> = schema::token::table
        .filter(schema::token::user_id.eq(query_user.id))
        .order((schema::token::creation, schema::token::expiration))
        .load::<QueryToken>(conn)
        .map_err(map_http_error!("Failed to get tokens."))?
        .into_iter()
        .filter_map(|query| query.to_json(conn).ok())
        .collect();

    Ok(json)
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/tokens",
    tags = ["tokens"]
}]
pub async fn post_options(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
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
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonToken>, CorsHeaders>, HttpError> {
    let user_id = QueryUser::auth(&rqctx).await?;
    let endpoint = Endpoint::new(TOKEN_RESOURCE, Method::Post);

    let context = rqctx.context();
    let json_token = body.into_inner();
    let json = post_inner(user_id, context, json_token)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

pub async fn post_inner(
    user_id: i32,
    context: &Context,
    json_token: JsonNewToken,
) -> Result<JsonToken, ApiError> {
    let context = &mut *context.lock().await;
    let conn = &mut context.db;
    let insert_token = InsertToken::from_json(conn, json_token, user_id, &context.key)?;
    diesel::insert_into(schema::token::table)
        .values(&insert_token)
        .execute(conn)
        .map_err(map_http_error!("Failed to create token."))?;

    schema::token::table
        .filter(schema::token::uuid.eq(&insert_token.uuid))
        .first::<QueryToken>(conn)
        .map_err(map_http_error!("Failed to create token."))?
        .to_json(conn)
        .map_err(Into::into)
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
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
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
) -> Result<HttpResponseHeaders<HttpResponseOk<JsonToken>, CorsHeaders>, HttpError> {
    let user_id = QueryUser::auth(&rqctx).await?;
    let path_params = path_params.into_inner();

    let context = &mut *rqctx.context().lock().await;
    let conn = &mut context.db;
    let query_user = QueryUser::from_resource_id(conn, &path_params.user)?;

    // TODO make smarter once permissions are a thing
    if query_user.id != user_id {
        return Err(http_error!("Failed to get token."));
    }

    let json = schema::token::table
        .filter(
            schema::token::user_id
                .eq(query_user.id)
                .and(schema::token::uuid.eq(&path_params.token.to_string())),
        )
        .first::<QueryToken>(conn)
        .map_err(map_http_error!("Failed to get token."))?
        .to_json(conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
    ))
}
