use bencher_endpoint::{
    CorsResponse, Delete, Endpoint, Get, ResponseDeleted, ResponseOk, TotalCount,
};
use bencher_json::{
    JsonDirection, JsonOAuthGrant, JsonOAuthGrants, JsonPagination, OAuthGrantUuid, UserResourceId,
};
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{forbidden_error, not_found_error, resource_conflict_err, resource_not_found_err},
    model::user::{QueryUser, auth::AuthUser, oauth::QueryOAuthGrant, same_user},
    schema, write_transaction,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{HttpError, Path, Query, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct UserOAuthGrantsParams {
    /// The slug or UUID for a user.
    pub user: UserResourceId,
}

pub type UserOAuthGrantsPagination = JsonPagination<UserOAuthGrantsSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserOAuthGrantsSort {
    /// Sort by when the app was connected.
    #[default]
    Created,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/users/{user}/oauth/grants",
    tags = ["users", "oauth"]
}]
pub async fn user_oauth_grants_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<UserOAuthGrantsParams>,
    _pagination_params: Query<UserOAuthGrantsPagination>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List connected apps
///
/// List the apps connected to a user's account through OAuth that can still act for the user.
/// Only the user themselves and server admins have access, with a JWT (user session token).
/// The HTTP response header `X-Total-Count` contains the total number of apps.
#[endpoint {
    method = GET,
    path =  "/v0/users/{user}/oauth/grants",
    tags = ["users", "oauth"]
}]
pub async fn user_oauth_grants_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<UserOAuthGrantsParams>,
    pagination_params: Query<UserOAuthGrantsPagination>,
) -> Result<ResponseOk<JsonOAuthGrants>, HttpError> {
    let auth_user = grants_user(&rqctx).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Get::auth_response_ok_with_total_count(json, total_count))
}

async fn get_ls_inner(
    context: &ApiContext,
    path_params: UserOAuthGrantsParams,
    pagination_params: UserOAuthGrantsPagination,
    auth_user: &AuthUser,
) -> Result<(JsonOAuthGrants, TotalCount), HttpError> {
    let query_user = QueryUser::from_resource_id(auth_conn!(context), &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.uuid);

    let now = context.clock.now();
    let query = QueryOAuthGrant::active(query_user.id, now);
    let query = match pagination_params.order() {
        UserOAuthGrantsSort::Created => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order((
                schema::oauth_grant::created.asc(),
                schema::oauth_grant::id.asc(),
            )),
            Some(JsonDirection::Desc) => query.order((
                schema::oauth_grant::created.desc(),
                schema::oauth_grant::id.desc(),
            )),
        },
    };
    let grants = query
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryOAuthGrant>(auth_conn!(context))
        .map_err(resource_not_found_err!(
            OAuthGrant,
            (&query_user, &pagination_params)
        ))?;

    let json_grants = auth_conn!(context, |conn| grants
        .into_iter()
        .map(|query_grant| query_grant.into_json(conn))
        .collect::<Result<Vec<_>, _>>()?);

    let total_count = QueryOAuthGrant::active(query_user.id, now)
        .count()
        .get_result::<i64>(auth_conn!(context))
        .map_err(resource_not_found_err!(
            OAuthGrant,
            (&query_user, &pagination_params)
        ))?
        .try_into()?;

    Ok((json_grants.into(), total_count))
}

#[derive(Deserialize, JsonSchema)]
pub struct UserOAuthGrantParams {
    /// The slug or UUID for a user.
    pub user: UserResourceId,
    /// The UUID for an OAuth grant.
    pub grant: OAuthGrantUuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/users/{user}/oauth/grants/{grant}",
    tags = ["users", "oauth"]
}]
pub async fn user_oauth_grant_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<UserOAuthGrantParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Delete.into()]))
}

/// View a connected app
///
/// View an app connected to a user's account through OAuth.
/// Only the user themselves and server admins have access, with a JWT (user session token).
#[endpoint {
    method = GET,
    path =  "/v0/users/{user}/oauth/grants/{grant}",
    tags = ["users", "oauth"]
}]
pub async fn user_oauth_grant_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<UserOAuthGrantParams>,
) -> Result<ResponseOk<JsonOAuthGrant>, HttpError> {
    let auth_user = grants_user(&rqctx).await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: UserOAuthGrantParams,
    auth_user: &AuthUser,
) -> Result<JsonOAuthGrant, HttpError> {
    let query_user = QueryUser::from_resource_id(auth_conn!(context), &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.uuid);

    auth_conn!(context, |conn| QueryOAuthGrant::get_active(
        conn,
        query_user.id,
        path_params.grant,
        context.clock.now()
    )?
    .into_json(conn))
}

/// Disconnect an app
///
/// Revoke the OAuth grant of an app connected to a user's account:
/// its tokens stop working at once, and it must ask for consent again.
/// Only the user themselves and server admins have access, with a JWT (user session token).
#[endpoint {
    method = DELETE,
    path =  "/v0/users/{user}/oauth/grants/{grant}",
    tags = ["users", "oauth"]
}]
pub async fn user_oauth_grant_delete(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<UserOAuthGrantParams>,
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = grants_user(&rqctx).await?;
    delete_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: UserOAuthGrantParams,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    let query_user = QueryUser::from_resource_id(auth_conn!(context), &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.uuid);

    let now = context.clock.now();
    let query_grant =
        QueryOAuthGrant::get_active(auth_conn!(context), query_user.id, path_params.grant, now)?;
    // A grant revoked between the lookup and here is already what the caller asked for.
    write_transaction!(context, |conn| QueryOAuthGrant::revoke(
        conn,
        query_grant.id,
        now
    ))
    .map_err(resource_conflict_err!(
        OAuthGrant,
        (&query_user, &query_grant)
    ))?;

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthGrantRevoke);

    Ok(())
}

/// Authenticate a request that manages OAuth grants, which exist only once OAuth is configured.
/// A user API key may not manage them, so a leaked key can neither see nor sever
/// the connections it did not make.
async fn grants_user(rqctx: &RequestContext<ApiContext>) -> Result<AuthUser, HttpError> {
    if rqctx.context().oauth.is_none() {
        return Err(not_found_error("OAuth is not configured on this server"));
    }
    let auth_user = AuthUser::new(rqctx).await?;
    // Checked only after authentication: an invalid key still gets the opaque `401`.
    if auth_user.user_key_id.is_some() {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthGrantBlocked);
        return Err(forbidden_error(
            "A user API key cannot be used to manage OAuth grants. Authenticate with a JWT (user session token) instead.",
        ));
    }
    Ok(auth_user)
}
