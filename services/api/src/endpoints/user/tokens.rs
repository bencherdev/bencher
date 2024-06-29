use bencher_json::{
    user::token::JsonUpdateToken, JsonDirection, JsonNewToken, JsonPagination, JsonToken,
    JsonTokens, ResourceId, ResourceName,
};
use diesel::{
    BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl, TextExpressionMethods,
};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    conn_lock,
    context::ApiContext,
    endpoints::{
        endpoint::{
            CorsLsResponse, CorsResponse, Get, Patch, Post, ResponseCreated, ResponseOk,
            ResponseOkLs,
        },
        Endpoint,
    },
    error::{resource_conflict_err, resource_not_found_err},
    model::user::{
        auth::{AuthUser, BearerToken},
        same_user,
        token::{InsertToken, QueryToken, UpdateToken},
        QueryUser, UserId,
    },
    schema,
    util::{headers::TotalCount, search::Search},
};

#[derive(Deserialize, JsonSchema)]
pub struct UserTokensParams {
    /// The slug or UUID for a user.
    pub user: ResourceId,
}

pub type UserTokensPagination = JsonPagination<UserTokensSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserTokensSort {
    /// Sort by token name.
    #[default]
    Name,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UserTokensQuery {
    /// Filter by token name, exact match.
    pub name: Option<ResourceName>,
    /// Search by token name, slug, or UUID.
    pub search: Option<Search>,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
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
) -> Result<CorsLsResponse, HttpError> {
    Ok(Endpoint::cors_ls(&[Get.into(), Post.into()]))
}

/// List tokens for a user
///
/// List all API tokens for a user.
/// Only the authenticated user themselves and server admins have access to this endpoint.
/// By default, the tokens are sorted in alphabetical order by name.
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
) -> Result<ResponseOkLs<JsonTokens>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Get::auth_response_ok_ls(json, total_count))
}

async fn get_ls_inner(
    context: &ApiContext,
    path_params: UserTokensParams,
    pagination_params: UserTokensPagination,
    query_params: UserTokensQuery,
    auth_user: &AuthUser,
) -> Result<(JsonTokens, TotalCount), HttpError> {
    let query_user = QueryUser::from_resource_id(conn_lock!(context), &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.uuid);

    let tokens = get_ls_query(&pagination_params, &query_params, query_user.id)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryToken>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Token,
            (&pagination_params, &query_params, auth_user)
        ))?
        .into_iter()
        .map(|query_token| query_token.into_json_for_user(auth_user))
        .collect();
    let total_count = get_ls_query(&pagination_params, &query_params, query_user.id)
        .count()
        .get_result::<i64>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Token,
            (&pagination_params, &query_params, auth_user)
        ))?
        .try_into()?;

    Ok((tokens, total_count))
}

fn get_ls_query<'q>(
    pagination_params: &UserTokensPagination,
    query_params: &'q UserTokensQuery,
    user_id: UserId,
) -> schema::token::BoxedQuery<'q, diesel::sqlite::Sqlite> {
    let mut query = schema::token::table
        .filter(schema::token::user_id.eq(user_id))
        .into_boxed();

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::token::name.eq(name));
    }
    if let Some(search) = query_params.search.as_ref() {
        query = query.filter(
            schema::token::name
                .like(search)
                .or(schema::token::uuid.like(search)),
        );
    }

    match pagination_params.order() {
        UserTokensSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => {
                query.order((schema::token::name.asc(), schema::token::expiration.asc()))
            },
            Some(JsonDirection::Desc) => {
                query.order((schema::token::name.desc(), schema::token::expiration.desc()))
            },
        },
    }
}

/// Create a token
///
/// Create an API token for a user.
/// Only the authenticated user themselves and server admins have access to this endpoint.
#[endpoint {
    method = POST,
    path =  "/v0/users/{user}/tokens",
    tags = ["users", "tokens"]
}]
pub async fn user_token_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<UserTokensParams>,
    body: TypedBody<JsonNewToken>,
) -> Result<ResponseCreated<JsonToken>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    context: &ApiContext,
    path_params: UserTokensParams,
    json_token: JsonNewToken,
    auth_user: &AuthUser,
) -> Result<JsonToken, HttpError> {
    let insert_token = InsertToken::from_json(
        conn_lock!(context),
        &context.rbac,
        &context.token_key,
        &path_params.user,
        json_token,
        auth_user,
    )?;

    diesel::insert_into(schema::token::table)
        .values(&insert_token)
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Token, insert_token))?;

    conn_lock!(context, |conn| schema::token::table
        .filter(schema::token::uuid.eq(&insert_token.uuid))
        .first::<QueryToken>(conn)
        .map_err(resource_not_found_err!(Token, insert_token))?
        .into_json(conn))
}

#[derive(Deserialize, JsonSchema)]
pub struct UserTokenParams {
    /// The slug or UUID for a user.
    pub user: ResourceId,
    /// The UUID for a token.
    pub token: Uuid,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/users/{user}/tokens/{token}",
    tags = ["users", "tokens"]
}]
pub async fn user_token_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<UserTokenParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into()]))
}

/// View a token
///
/// View an API token for a user.
/// Only the authenticated user themselves and server admins have access to this endpoint.
#[endpoint {
    method = GET,
    path =  "/v0/users/{user}/tokens/{token}",
    tags = ["users", "tokens"]
}]
pub async fn user_token_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<UserTokenParams>,
) -> Result<ResponseOk<JsonToken>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: UserTokenParams,
    auth_user: &AuthUser,
) -> Result<JsonToken, HttpError> {
    let query_user = QueryUser::from_resource_id(conn_lock!(context), &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.uuid);

    conn_lock!(context, |conn| QueryToken::get_user_token(
        conn,
        query_user.id,
        &path_params.token.to_string()
    )?
    .into_json(conn)
    .map_err(resource_not_found_err!(
        Token,
        (&query_user, path_params.token)
    )))
}

/// Update a token
///
/// Update an API token for a user.
/// Only the authenticated user themselves and server admins have access to this endpoint.
#[endpoint {
    method = PATCH,
    path =  "/v0/users/{user}/tokens/{token}",
    tags = ["users", "tokens"]
}]
pub async fn user_token_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<UserTokenParams>,
    body: TypedBody<JsonUpdateToken>,
) -> Result<ResponseOk<JsonToken>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = patch_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Patch::auth_response_ok(json))
}

async fn patch_inner(
    context: &ApiContext,
    path_params: UserTokenParams,
    json_token: JsonUpdateToken,
    auth_user: &AuthUser,
) -> Result<JsonToken, HttpError> {
    let query_user = QueryUser::from_resource_id(conn_lock!(context), &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.uuid);

    let query_token = QueryToken::get_user_token(
        conn_lock!(context),
        query_user.id,
        &path_params.token.to_string(),
    )?;

    let update_token = UpdateToken::from(json_token);
    diesel::update(schema::token::table.filter(schema::token::id.eq(query_token.id)))
        .set(&update_token)
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Token, (&query_user, &query_token)))?;

    conn_lock!(context, |conn| QueryToken::get(conn, query_token.id)?
        .into_json(conn))
}
