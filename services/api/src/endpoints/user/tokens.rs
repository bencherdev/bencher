use bencher_json::{
    user::token::JsonUpdateToken, JsonDirection, JsonNewToken, JsonPagination, JsonToken,
    JsonTokens, NonEmpty, ResourceId,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{response_accepted, response_ok, CorsResponse, ResponseAccepted, ResponseOk},
        Endpoint,
    },
    model::{
        user::QueryUser,
        user::{
            auth::AuthUser,
            token::{same_user, InsertToken, QueryToken, UpdateToken},
        },
    },
    schema,
    util::error::into_json,
    ApiError,
};

#[derive(Deserialize, JsonSchema)]
pub struct UserTokensParams {
    pub user: ResourceId,
}

pub type UserTokensPagination = JsonPagination<UserTokensSort>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserTokensSort {
    #[default]
    Name,
}

#[derive(Deserialize, JsonSchema)]
pub struct UserTokensQuery {
    pub name: Option<NonEmpty>,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/users/{user}/tokens",
    tags = ["users", "tokens"]
}]
pub async fn user_tokens_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<UserTokensParams>,
    _pagination_params: Query<UserTokensPagination>,
    _query_params: Query<UserTokensQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Endpoint::GetLs, Endpoint::Post]))
}

#[endpoint {
    method = GET,
    path =  "/v0/users/{user}/tokens",
    tags = ["users", "tokens"]
}]
pub async fn user_tokens_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<UserTokensParams>,
    pagination_params: Query<UserTokensPagination>,
    query_params: Query<UserTokensQuery>,
) -> Result<ResponseOk<JsonTokens>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::GetLs;

    let context = rqctx.context();
    let json = get_ls_inner(
        context,
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params.into_inner(),
        &auth_user,
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

    response_ok!(endpoint, json)
}

async fn get_ls_inner(
    context: &ApiContext,
    path_params: UserTokensParams,
    pagination_params: UserTokensPagination,
    query_params: UserTokensQuery,
    auth_user: &AuthUser,
    endpoint: Endpoint,
) -> Result<JsonTokens, ApiError> {
    let conn = &mut *context.conn().await;

    let query_user = QueryUser::from_resource_id(conn, &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.id);

    let mut query = schema::token::table
        .filter(schema::token::user_id.eq(query_user.id))
        .into_boxed();

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::token::name.eq(name.as_ref()));
    }

    query = match pagination_params.order() {
        UserTokensSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => {
                query.order((schema::token::name.asc(), schema::token::expiration.asc()))
            },
            Some(JsonDirection::Desc) => {
                query.order((schema::token::name.desc(), schema::token::expiration.desc()))
            },
        },
    };

    Ok(query
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryToken>(conn)
        .map_err(ApiError::from)?
        .into_iter()
        .filter_map(into_json!(endpoint, conn))
        .collect())
}

#[endpoint {
    method = POST,
    path =  "/v0/users/{user}/tokens",
    tags = ["users", "tokens"]
}]
pub async fn user_token_post(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<UserTokensParams>,
    body: TypedBody<JsonNewToken>,
) -> Result<ResponseAccepted<JsonToken>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::Post;

    let context = rqctx.context();
    let json_token = body.into_inner();
    let json = post_inner(context, path_params.into_inner(), json_token, &auth_user)
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
    path_params: UserTokensParams,
    json_token: JsonNewToken,
    auth_user: &AuthUser,
) -> Result<JsonToken, ApiError> {
    let conn = &mut *context.conn().await;

    let insert_token = InsertToken::from_json(
        conn,
        &context.rbac,
        &context.secret_key,
        &path_params.user,
        json_token,
        auth_user,
    )?;

    diesel::insert_into(schema::token::table)
        .values(&insert_token)
        .execute(conn)
        .map_err(ApiError::from)?;

    schema::token::table
        .filter(schema::token::uuid.eq(&insert_token.uuid))
        .first::<QueryToken>(conn)
        .map_err(ApiError::from)?
        .into_json(conn)
        .map_err(ApiError::from)
}

#[derive(Deserialize, JsonSchema)]
pub struct UserTokenParams {
    pub user: ResourceId,
    pub token: Uuid,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/users/{user}/tokens/{token}",
    tags = ["users", "tokens"]
}]
pub async fn user_token_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<UserTokenParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Endpoint::GetOne, Endpoint::Patch]))
}

#[endpoint {
    method = GET,
    path =  "/v0/users/{user}/tokens/{token}",
    tags = ["users", "tokens"]
}]
pub async fn user_token_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<UserTokenParams>,
) -> Result<ResponseOk<JsonToken>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::GetOne;

    let context = rqctx.context();
    let path_params = path_params.into_inner();
    let json = get_one_inner(context, path_params, &auth_user)
        .await
        .map_err(|e| {
            if let ApiError::HttpError(e) = e {
                e
            } else {
                endpoint.err(e).into()
            }
        })?;

    response_ok!(endpoint, json)
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: UserTokenParams,
    auth_user: &AuthUser,
) -> Result<JsonToken, ApiError> {
    let conn = &mut *context.conn().await;

    let query_user = QueryUser::from_resource_id(conn, &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.id);

    QueryToken::get_user_token(conn, query_user.id, &path_params.token.to_string())?
        .into_json(conn)
        .map_err(ApiError::from)
}

#[endpoint {
    method = PATCH,
    path =  "/v0/users/{user}/tokens/{token}",
    tags = ["users", "tokens"]
}]
pub async fn user_token_patch(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<UserTokenParams>,
    body: TypedBody<JsonUpdateToken>,
) -> Result<ResponseAccepted<JsonToken>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::Patch;

    let context = rqctx.context();
    let json = patch_inner(
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

async fn patch_inner(
    context: &ApiContext,
    path_params: UserTokenParams,
    json_token: JsonUpdateToken,
    auth_user: &AuthUser,
) -> Result<JsonToken, ApiError> {
    let conn = &mut *context.conn().await;

    let query_user = QueryUser::from_resource_id(conn, &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.id);

    let query_token =
        QueryToken::get_user_token(conn, query_user.id, &path_params.token.to_string())?;

    diesel::update(schema::token::table.filter(schema::token::id.eq(query_token.id)))
        .set(&UpdateToken::from(json_token))
        .execute(conn)
        .map_err(ApiError::from)?;

    QueryToken::get(conn, query_token.id)?.into_json(conn)
}
