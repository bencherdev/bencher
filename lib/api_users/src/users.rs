use bencher_endpoint::{CorsResponse, Endpoint, Get, Patch, ResponseOk, TotalCount};
use bencher_json::{
    DateTime, JsonDirection, JsonPagination, JsonUpdateUser, JsonUser, Sanitize as _, Search,
    UserName, UserResourceId, user::JsonUsers,
};
use bencher_schema::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::{forbidden_error, resource_conflict_err, resource_not_found_err},
    model::user::{
        QueryUser, UpdateUser, UserId,
        admin::AdminUser,
        auth::{AuthUser, BearerToken},
        email_change::{InsertEmailChange, QueryEmailChange},
        same_user,
        session::QuerySession,
        token::QueryToken,
    },
    schema, write_transaction,
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    TextExpressionMethods as _,
};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;
use slog::Logger;

use crate::email::{check_new_email, send_asked, send_changed, taken};

pub type UsersPagination = JsonPagination<UsersSort>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UsersSort {
    /// Sort by user name.
    #[default]
    Name,
}

#[derive(Deserialize, JsonSchema)]
pub struct UsersQuery {
    /// Filter by user name, exact match.
    pub name: Option<UserName>,
    /// Search by user name, slug, or UUID.
    pub search: Option<Search>,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/users",
    tags = ["users"]
}]
pub async fn users_options(
    _rqctx: RequestContext<ApiContext>,
    _pagination_params: Query<UsersPagination>,
    _query_params: Query<UsersQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List users
///
/// List all users.
/// The user must be an admin on the server to use this route.
/// The HTTP response header `X-Total-Count` contains the total number of users.
#[endpoint {
    method = GET,
    path =  "/v0/users",
    tags = ["users"]
}]
pub async fn users_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    pagination_params: Query<UsersPagination>,
    query_params: Query<UsersQuery>,
) -> Result<ResponseOk<JsonUsers>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        pagination_params.into_inner(),
        query_params.into_inner(),
    )
    .await?;
    Ok(Get::auth_response_ok_with_total_count(json, total_count))
}

async fn get_ls_inner(
    context: &ApiContext,
    pagination_params: UsersPagination,
    query_params: UsersQuery,
) -> Result<(JsonUsers, TotalCount), HttpError> {
    let users = get_ls_query(&pagination_params, &query_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryUser>(auth_conn!(context))
        .map_err(resource_not_found_err!(User))?;

    // Drop connection lock before iterating
    let json_users = users.into_iter().map(QueryUser::into_json).collect();

    let total_count = get_ls_query(&pagination_params, &query_params)
        .count()
        .get_result::<i64>(auth_conn!(context))
        .map_err(resource_not_found_err!(User))?
        .try_into()?;

    Ok((json_users, total_count))
}

fn get_ls_query<'q>(
    pagination_params: &UsersPagination,
    query_params: &'q UsersQuery,
) -> schema::user::BoxedQuery<'q, diesel::sqlite::Sqlite> {
    let mut query = schema::user::table.into_boxed();

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::user::name.eq(name));
    }
    if let Some(search) = query_params.search.as_ref() {
        query = query.filter(
            schema::user::name
                .like(search)
                .or(schema::user::slug.like(search))
                .or(schema::user::uuid.like(search)),
        );
    }

    match pagination_params.order() {
        UsersSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => {
                query.order((schema::user::name.asc(), schema::user::slug.asc()))
            },
            Some(JsonDirection::Desc) => {
                query.order((schema::user::name.desc(), schema::user::slug.desc()))
            },
        },
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct UserParams {
    /// The slug or UUID for a user.
    pub user: UserResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/users/{user}",
    tags = ["users"]
}]
pub async fn user_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<UserParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into()]))
}

/// View a user
///
/// View a user.
/// Only the authenticated user themselves and server admins have access to this endpoint.
/// To view users within your organization, use the organization members endpoints.
#[endpoint {
    method = GET,
    path =  "/v0/users/{user}",
    tags = ["users"]
}]
pub async fn user_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<UserParams>,
) -> Result<ResponseOk<JsonUser>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: UserParams,
    auth_user: &AuthUser,
) -> Result<JsonUser, HttpError> {
    let query_user = QueryUser::from_resource_id(auth_conn!(context), &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.uuid);

    Ok(query_user.into_json())
}

/// Update a user
///
/// Update a user.
/// Only the authenticated user themselves and server admins have access to this endpoint.
/// Some fields can only be updated by an admin.
/// Changing the email, admin, or locked field requires a user session, even for an admin:
/// neither a user API key nor a deprecated user API token can change them.
/// Changing any of them also needs a sign-in within the last 10 minutes.
/// A user's new email for themselves takes effect only once it is confirmed
/// from a link sent to that email, good once in the next 30 minutes,
/// so until then the response shows the current email; the current email is told of it at once.
/// Asking again ends the earlier link.
/// Confirming revokes the user's deprecated API tokens and ends every session of theirs
/// but the one that asked.
/// A server admin's change to another user's email takes effect at once:
/// both emails are told of it, the user's deprecated API tokens are revoked,
/// and every session of theirs ends.
/// That change, or a lock, also ends any email change the user asked for.
/// To manage users within your organization, use the organization members endpoints.
#[endpoint {
    method = PATCH,
    path =  "/v0/users/{user}",
    tags = ["users"]
}]
pub async fn user_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<UserParams>,
    body: TypedBody<JsonUpdateUser>,
) -> Result<ResponseOk<JsonUser>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = patch_inner(
        &rqctx.log,
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Patch::auth_response_ok(json))
}

async fn patch_inner(
    log: &Logger,
    context: &ApiContext,
    path_params: UserParams,
    json_user: JsonUpdateUser,
    auth_user: &AuthUser,
) -> Result<JsonUser, HttpError> {
    let query_user = QueryUser::from_resource_id(auth_conn!(context), &path_params.user)?;
    same_user!(auth_user, context.rbac, query_user.uuid);

    let admin_only_error = |field: &str| {
        let mut auth_user = auth_user.clone();
        auth_user.sanitize();
        forbidden_error(format!(
            "Only admins can update the `{field}` field for a user. User is not an admin: {auth_user:?}",
        ))
    };
    if json_user.admin.is_some() && !auth_user.is_admin(&context.rbac) {
        return Err(admin_only_error("admin"));
    }
    if json_user.locked.is_some() && !auth_user.is_admin(&context.rbac) {
        return Err(admin_only_error("locked"));
    }

    let email_changed = json_user
        .email
        .as_ref()
        .is_some_and(|new_email| *new_email != query_user.email);
    let admin_changed = json_user
        .admin
        .is_some_and(|new_admin| new_admin != query_user.admin);
    let locked_changed = json_user
        .locked
        .is_some_and(|new_locked| new_locked != query_user.locked);
    if !auth_user.is_session() {
        if email_changed {
            return Err(forbidden_error(
                "Only a user session can change a user's email.",
            ));
        }
        if admin_changed {
            return Err(forbidden_error(
                "Only a user session can change whether a user is an admin.",
            ));
        }
        if locked_changed {
            return Err(forbidden_error(
                "Only a user session can lock or unlock a user.",
            ));
        }
    }
    if email_changed || admin_changed || locked_changed {
        auth_user.check_fresh_sign_in(context).await?;
    }

    let mut update_user = UpdateUser::from(json_user.clone());
    // Unchanged as read, so never written: a stale address must not land on a newer, proven one.
    if !email_changed {
        update_user.email = None;
    }
    let now = context.clock.now();
    // A user's own new email waits for its confirm link; a server admin's for another user is set
    // here.
    let asked = if email_changed && auth_user.uuid() == query_user.uuid {
        update_user.email.take()
    } else {
        None
    };
    let asked = if let Some(new_email) = asked {
        check_new_email(context, &query_user, &new_email).await?;
        Some(InsertEmailChange::new(
            query_user.id,
            auth_user.session_id(),
            query_user.email.clone(),
            new_email,
            now,
        )?)
    } else {
        None
    };

    let written = write_transaction!(context, |conn| write_update(
        conn,
        query_user.id,
        &update_user,
        asked.as_ref().map(|(change, _secret)| change),
        now
    ))
    .map_err(resource_conflict_err!(User, (&query_user, &json_user)))?;
    if !written && let Some(new_email) = &update_user.email {
        return Err(taken(&query_user, new_email));
    }
    let user = QueryUser::get(auth_conn!(context), query_user.id)?;

    if let Some((change, secret)) = &asked {
        send_asked(log, context, &user, &change.new_email, secret).await?;
    }
    if update_user.email.is_some() {
        send_changed(log, context, &user, &query_user.email).await?;
    }

    Ok(user.into_json())
}

/// Write the update to the user, and the user's own email change if they asked for one, unless
/// the email a server admin sets is another user's.
///
/// A server admin's change of the email ends every session of the user, their pending email
/// change, and their deprecated API tokens, as a lock ends every session and the pending change.
fn write_update(
    conn: &mut DbConnection,
    user_id: UserId,
    update_user: &UpdateUser,
    change: Option<&InsertEmailChange>,
    now: DateTime,
) -> diesel::QueryResult<bool> {
    if let Some(new_email) = &update_user.email
        && diesel::select(diesel::dsl::exists(
            schema::user::table.filter(schema::user::email.eq(new_email)),
        ))
        .get_result::<bool>(conn)?
    {
        return Ok(false);
    }
    // An email set here, by an admin for another user, is unproven.
    let unverified = update_user
        .email
        .is_some()
        .then_some(schema::user::email_verified.eq(None::<DateTime>));
    diesel::update(schema::user::table.filter(schema::user::id.eq(user_id)))
        .set((update_user, schema::user::modified.eq(now), unverified))
        .execute(conn)?;
    if let Some(change) = change {
        change.upsert(conn)?;
    }
    if update_user.email.is_some() {
        QueryToken::revoke_all(conn, user_id, now)?;
    }
    if update_user.email.is_some() || update_user.locked == Some(true) {
        QuerySession::delete_all(conn, user_id)?;
        QueryEmailChange::end(conn, user_id)?;
    }
    Ok(true)
}
