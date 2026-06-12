use bencher_endpoint::{
    CorsResponse, Delete, Endpoint, Get, Patch, Post, ResponseCreated, ResponseDeleted, ResponseOk,
    TotalCount,
};
use bencher_json::{
    JsonDirection, JsonNewUserKey, JsonPagination, JsonUserKey, JsonUserKeyCreated, JsonUserKeys,
    ResourceName, Search, UserKeyUuid, UserResourceId, user::key::JsonUpdateUserKey,
};
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{conflict_error, forbidden_error, resource_conflict_err, resource_not_found_err},
    model::user::{
        QueryUser, UserId,
        auth::{AuthUser, BearerToken},
        key::{InsertUserKey, QueryUserKey, UpdateUserKey, UserKeyId},
        same_user,
    },
    schema, write_conn, write_transaction,
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    TextExpressionMethods as _,
};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct UserKeysParams {
    /// The slug or UUID for a user.
    pub user: UserResourceId,
}

pub type UserKeysPagination = JsonPagination<UserKeysSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserKeysSort {
    /// Sort by key name.
    #[default]
    Name,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UserKeysQuery {
    /// Filter by key name, exact match.
    pub name: Option<ResourceName>,
    /// Search by key name or UUID.
    pub search: Option<Search>,
    /// If set to `true`, only returns revoked keys.
    /// If not set or set to `false`, only returns non-revoked keys.
    pub revoked: Option<bool>,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/users/{user}/keys",
    tags = ["users", "keys"]
}]
pub async fn user_keys_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<UserKeysParams>,
    _pagination_params: Query<UserKeysPagination>,
    _query_params: Query<UserKeysQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// List user API keys
///
/// List all `bencher_user_*` API keys for a user.
/// Only the authenticated user themselves and server admins have access to this endpoint.
/// When authenticated with a user API key, only that key itself is listed:
/// a key cannot enumerate the user's other keys.
/// By default, the keys are sorted in alphabetical order by name.
/// The HTTP response header `X-Total-Count` contains the total number of keys.
#[endpoint {
    method = GET,
    path =  "/v0/users/{user}/keys",
    tags = ["users", "keys"]
}]
pub async fn user_keys_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<UserKeysParams>,
    pagination_params: Query<UserKeysPagination>,
    query_params: Query<UserKeysQuery>,
) -> Result<ResponseOk<JsonUserKeys>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Get::auth_response_ok_with_total_count(json, total_count))
}

async fn get_ls_inner(
    context: &ApiContext,
    path_params: UserKeysParams,
    pagination_params: UserKeysPagination,
    query_params: UserKeysQuery,
    auth_user: &AuthUser,
) -> Result<(JsonUserKeys, TotalCount), HttpError> {
    let query_user = QueryUser::from_resource_id(auth_conn!(context), &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.uuid);

    let keys = get_ls_query(
        &pagination_params,
        &query_params,
        query_user.id,
        auth_user.user_key_id,
    )
    .offset(pagination_params.offset())
    .limit(pagination_params.limit())
    .load::<QueryUserKey>(auth_conn!(context))
    .map_err(resource_not_found_err!(
        UserKey,
        (&pagination_params, &query_params, auth_user)
    ))?;

    let json_keys: JsonUserKeys = keys
        .into_iter()
        .map(|query_key| query_key.into_json_for_user(&query_user))
        .collect();

    let total_count = get_ls_query(
        &pagination_params,
        &query_params,
        query_user.id,
        auth_user.user_key_id,
    )
    .count()
    .get_result::<i64>(auth_conn!(context))
    .map_err(resource_not_found_err!(
        UserKey,
        (&pagination_params, &query_params, auth_user)
    ))?
    .try_into()?;

    Ok((json_keys, total_count))
}

fn get_ls_query<'q>(
    pagination_params: &UserKeysPagination,
    query_params: &'q UserKeysQuery,
    user_id: UserId,
    auth_key_id: Option<UserKeyId>,
) -> schema::user_key::BoxedQuery<'q, diesel::sqlite::Sqlite> {
    let mut query = schema::user_key::table
        .filter(schema::user_key::user_id.eq(user_id))
        .into_boxed();

    // A user key can only see itself.
    if let Some(auth_key_id) = auth_key_id {
        query = query.filter(schema::user_key::id.eq(auth_key_id));
    }

    if let Some(true) = query_params.revoked {
        query = query.filter(schema::user_key::revoked.is_not_null());
    } else {
        query = query.filter(schema::user_key::revoked.is_null());
    }

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::user_key::name.eq(name));
    }
    if let Some(search) = query_params.search.as_ref() {
        query = query.filter(
            schema::user_key::name
                .like(search)
                .or(schema::user_key::uuid.like(search)),
        );
    }

    match pagination_params.order() {
        UserKeysSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order((
                schema::user_key::name.asc(),
                schema::user_key::creation.asc(),
            )),
            Some(JsonDirection::Desc) => query.order((
                schema::user_key::name.desc(),
                schema::user_key::creation.desc(),
            )),
        },
    }
}

/// Create a user API key
///
/// Create a new `bencher_user_*` API key for a user.
/// The plaintext key is only returned once in the response.
/// Only the authenticated user themselves and server admins have access to this endpoint.
/// This endpoint cannot be called with a user API key:
/// a key cannot be used to create another key, so revoking a key
/// also cuts off any credentials that could have been derived from it.
/// Authenticate with an API token (JWT) instead.
#[endpoint {
    method = POST,
    path =  "/v0/users/{user}/keys",
    tags = ["users", "keys"]
}]
pub async fn user_key_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<UserKeysParams>,
    body: TypedBody<JsonNewUserKey>,
) -> Result<ResponseCreated<JsonUserKeyCreated>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    // Check only after successful authentication: an invalid or revoked key
    // still gets the opaque `Invalid user key` 401, while a valid key gets a
    // 403 that explains the policy.
    if auth_user.user_key_id.is_some() {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserKeyCreateBlocked);
        return Err(forbidden_error(
            "A user API key cannot be used to create another user API key. Authenticate with an API token (JWT) instead.",
        ));
    }
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
    path_params: UserKeysParams,
    json_key: JsonNewUserKey,
    auth_user: &AuthUser,
) -> Result<JsonUserKeyCreated, HttpError> {
    let query_user = QueryUser::from_resource_id(auth_conn!(context), &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.uuid);

    #[cfg(feature = "plus")]
    context
        .rate_limiting
        .create_credential(auth_user.user.uuid)?;

    let now = context.clock.now();
    let (insert_key, plaintext_key) = InsertUserKey::from_json(query_user.id, json_key, now)?;

    diesel::insert_into(schema::user_key::table)
        .values(&insert_key)
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(UserKey, insert_key))?;

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserKeyCreate);

    Ok(insert_key.into_json(query_user.uuid, plaintext_key))
}

#[derive(Deserialize, JsonSchema)]
pub struct UserKeyParams {
    /// The slug or UUID for a user.
    pub user: UserResourceId,
    /// The UUID for a user key.
    pub key: UserKeyUuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/users/{user}/keys/{key}",
    tags = ["users", "keys"]
}]
pub async fn user_key_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<UserKeyParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into(), Delete.into()]))
}

/// View a user API key
///
/// View an API key for a user.
/// Only the authenticated user themselves and server admins have access to this endpoint.
/// When authenticated with a user API key, only that key itself may be viewed:
/// a key cannot inspect the user's other keys.
/// Authenticate with an API token (JWT) to view other keys.
#[endpoint {
    method = GET,
    path =  "/v0/users/{user}/keys/{key}",
    tags = ["users", "keys"]
}]
pub async fn user_key_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<UserKeyParams>,
) -> Result<ResponseOk<JsonUserKey>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: UserKeyParams,
    auth_user: &AuthUser,
) -> Result<JsonUserKey, HttpError> {
    let query_user = QueryUser::from_resource_id(auth_conn!(context), &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.uuid);

    let query_key =
        QueryUserKey::get_user_key(auth_conn!(context), query_user.id, path_params.key)?;

    // A user key can only see itself.
    if let Some(auth_key_id) = auth_user.user_key_id
        && auth_key_id != query_key.id
    {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserKeyViewBlocked);
        return Err(forbidden_error(
            "A user API key can only view itself. Authenticate with an API token (JWT) to view other keys.",
        ));
    }

    Ok(query_key.into_json_for_user(&query_user))
}

/// Update a user API key
///
/// Update an API key for a user (rename only).
/// Only the authenticated user themselves and server admins have access to this endpoint.
/// When authenticated with a user API key, only that key itself may be updated:
/// a key cannot modify the user's other keys.
/// Authenticate with an API token (JWT) to update other keys.
#[endpoint {
    method = PATCH,
    path =  "/v0/users/{user}/keys/{key}",
    tags = ["users", "keys"]
}]
pub async fn user_key_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<UserKeyParams>,
    body: TypedBody<JsonUpdateUserKey>,
) -> Result<ResponseOk<JsonUserKey>, HttpError> {
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
    path_params: UserKeyParams,
    json_key: JsonUpdateUserKey,
    auth_user: &AuthUser,
) -> Result<JsonUserKey, HttpError> {
    let query_user = QueryUser::from_resource_id(auth_conn!(context), &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.uuid);

    let query_key =
        QueryUserKey::get_user_key(auth_conn!(context), query_user.id, path_params.key)?;

    // A user key may only mutate itself.
    if let Some(auth_key_id) = auth_user.user_key_id
        && auth_key_id != query_key.id
    {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserKeyUpdateBlocked);
        return Err(forbidden_error(
            "A user API key can only update itself. Authenticate with an API token (JWT) to update other keys.",
        ));
    }

    let update_key = UpdateUserKey::from(json_key);
    write_transaction!(context, |conn| {
        diesel::update(schema::user_key::table.filter(schema::user_key::id.eq(query_key.id)))
            .set(&update_key)
            .execute(conn)
    })
    .map_err(resource_conflict_err!(UserKey, (&query_user, &query_key)))?;

    auth_conn!(context, |conn| {
        let query_key = QueryUserKey::get(conn, query_key.id)?;
        Ok(query_key.into_json_for_user(&query_user))
    })
}

/// Revoke a user API key
///
/// Revoke an API key for a user.
/// Revocation is terminal: a revoked key can no longer authenticate any request,
/// and the revocation cannot be undone.
/// Only the authenticated user themselves and server admins have access to this endpoint.
/// When authenticated with a user API key, only that key itself may be revoked:
/// a key can always destroy itself, but it cannot revoke the user's other keys.
/// Authenticate with an API token (JWT) to revoke other keys.
#[endpoint {
    method = DELETE,
    path =  "/v0/users/{user}/keys/{key}",
    tags = ["users", "keys"]
}]
pub async fn user_key_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<UserKeyParams>,
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: UserKeyParams,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    let query_user = QueryUser::from_resource_id(auth_conn!(context), &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.uuid);

    let query_key =
        QueryUserKey::get_user_key(auth_conn!(context), query_user.id, path_params.key)?;

    // A user key may always revoke itself (burn-after-use, in-CI incident
    // response) but never the user's other keys: a stolen key revoking its
    // siblings is a denial-of-service vector with no legitimate workflow.
    if let Some(auth_key_id) = auth_user.user_key_id
        && auth_key_id != query_key.id
    {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserKeyRevokeBlocked);
        return Err(forbidden_error(
            "A user API key can only revoke itself. Authenticate with an API token (JWT) to revoke other keys.",
        ));
    }

    let now = context.clock.now();
    let rows = write_transaction!(context, |conn| QueryUserKey::revoke(
        conn,
        query_key.id,
        now
    ))
    .map_err(resource_conflict_err!(UserKey, (&query_user, &query_key)))?;
    if rows == 0 {
        return Err(conflict_error("User key has already been revoked"));
    }

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserKeyRevoke);

    Ok(())
}
