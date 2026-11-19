use bencher_endpoint::{
    CorsResponse, Delete, Endpoint, Get, Patch, Post, ResponseAccepted, ResponseDeleted,
    ResponseOk, TotalCount,
};
use bencher_json::{
    Email, JsonAuthAck, JsonDirection, JsonMember, JsonMembers, JsonPagination,
    OrganizationResourceId, Search, UserName, UserResourceId,
    organization::member::{JsonNewMember, JsonUpdateMember},
};
use bencher_rbac::organization::Permission;
use bencher_schema::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::{
        conflict_error, forbidden_error, issue_error, resource_conflict_err, resource_not_found_err,
    },
    model::{
        organization::{
            OrganizationId, QueryOrganization, invite::InsertInvite, member::QueryMember,
        },
        user::{
            QueryUser, UserId,
            auth::{AuthUser, BearerToken},
        },
    },
    schema, write_conn,
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    TextExpressionMethods as _,
};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;
use slog::Logger;

use crate::invites::{check_inviter, email_invite};

#[derive(Deserialize, JsonSchema)]
pub struct OrgMembersParams {
    /// The slug or UUID for an organization.
    pub organization: OrganizationResourceId,
}

pub type OrgMembersPagination = JsonPagination<OrgMembersSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrgMembersSort {
    /// Sort by user name.
    #[default]
    Name,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OrgMembersQuery {
    /// Filter by user name, exact match.
    pub name: Option<UserName>,
    /// Search by user name, slug, or UUID.
    pub search: Option<Search>,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/members",
    tags = ["organizations", "members"]
}]
pub async fn org_members_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrgMembersParams>,
    _pagination_params: Query<OrgMembersPagination>,
    _query_params: Query<OrgMembersQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// List organization members
///
/// List members for an organization.
/// The user must have `view_role` permissions for the organization.
/// By default, the members are sorted in alphabetical order by name.
/// The HTTP response header `X-Total-Count` contains the total number of members.
#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/members",
    tags = ["organizations", "members"]
}]
pub async fn org_members_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OrgMembersParams>,
    pagination_params: Query<OrgMembersPagination>,
    query_params: Query<OrgMembersQuery>,
) -> Result<ResponseOk<JsonMembers>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        &auth_user,
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params.into_inner(),
    )
    .await?;
    Ok(Get::auth_response_ok_with_total_count(json, total_count))
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: &AuthUser,
    path_params: OrgMembersParams,
    pagination_params: OrgMembersPagination,
    query_params: OrgMembersQuery,
) -> Result<(JsonMembers, TotalCount), HttpError> {
    let query_organization = QueryOrganization::is_allowed_resource_id(
        auth_conn!(context),
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::ViewRole,
    )?;

    let members = get_ls_query(&query_organization, &pagination_params, &query_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryMember>(auth_conn!(context))
        .map_err(resource_not_found_err!(
            OrganizationRole,
            (&query_organization, &pagination_params, &query_params)
        ))?;

    // Drop connection lock before iterating
    let json_members = members.into_iter().map(QueryMember::into_json).collect();

    let total_count = get_ls_query(&query_organization, &pagination_params, &query_params)
        .count()
        .get_result::<i64>(auth_conn!(context))
        .map_err(resource_not_found_err!(
            OrganizationRole,
            (&query_organization, &pagination_params, &query_params)
        ))?
        .try_into()?;

    Ok((json_members, total_count))
}

fn get_ls_query<'q>(
    query_organization: &QueryOrganization,
    pagination_params: &OrgMembersPagination,
    query_params: &'q OrgMembersQuery,
) -> BoxedQuery<'q> {
    let mut query = schema::user::table
        .inner_join(schema::organization_role::table)
        .filter(schema::organization_role::organization_id.eq(query_organization.id))
        .select((
            schema::user::uuid,
            schema::user::name,
            schema::user::slug,
            schema::user::email,
            schema::organization_role::role,
            schema::organization_role::created,
            schema::organization_role::modified,
        ))
        .into_boxed();

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
        OrgMembersSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => {
                query.order((schema::user::name.asc(), schema::user::slug.asc()))
            },
            Some(JsonDirection::Desc) => {
                query.order((schema::user::name.desc(), schema::user::slug.desc()))
            },
        },
    }
}

// TODO refactor out internal types
type BoxedQuery<'q> = diesel::internal::table_macro::BoxedSelectStatement<
    'q,
    (
        diesel::sql_types::Text,
        diesel::sql_types::Text,
        diesel::sql_types::Text,
        diesel::sql_types::Text,
        diesel::sql_types::Text,
        diesel::sql_types::BigInt,
        diesel::sql_types::BigInt,
    ),
    diesel::internal::table_macro::FromClause<
        diesel::helper_types::InnerJoinQuerySource<
            schema::user::table,
            schema::organization_role::table,
        >,
    >,
    diesel::sqlite::Sqlite,
>;

/// Invite a user to an organization
///
/// Invite another user to become a member of an organization.
/// The user must have `create_role` permissions for the organization.
/// The invitee is sent an email with a link to accept the invitation, and
/// they are not added to the organization until they accept the invitation.
/// The link works once, for 7 days, and only for the invited email.
/// Inviting an email that the organization already invited sends a new link,
/// and the link it sent before no longer works.
/// An email that already belongs to a member of the organization is a conflict.
/// This endpoint requires a user session signed in within the last 10 minutes:
/// neither a user API key nor a deprecated user API token can send an invitation.
#[endpoint {
    method = POST,
    path =  "/v0/organizations/{organization}/members",
    tags = ["organizations", "members"]
}]
pub async fn org_member_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrgMembersParams>,
    body: TypedBody<JsonNewMember>,
) -> Result<ResponseAccepted<JsonAuthAck>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    check_inviter(rqctx.context(), &auth_user).await?;
    let json = post_inner(
        &rqctx.log,
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Post::auth_response_accepted(json))
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    path_params: OrgMembersParams,
    json_new_member: JsonNewMember,
    auth_user: &AuthUser,
) -> Result<JsonAuthAck, HttpError> {
    // Get the organization
    let query_org =
        QueryOrganization::from_resource_id(auth_conn!(context), &path_params.organization)?;

    // Check to see if user has permission to create a project within the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::CreateRole, &query_org)
        .map_err(forbidden_error)?;
    // Check the rate limit for sending invites
    #[cfg(feature = "plus")]
    auth_user.rate_limit_invites(context)?;

    let JsonNewMember { name, email, role } = json_new_member;
    if is_member(auth_conn!(context), query_org.id, &email)? {
        return Err(conflict_error(
            "This email already belongs to a member of the organization",
        ));
    }
    let (query_invite, secret) = InsertInvite::invite(
        write_conn!(context),
        query_org.id,
        email,
        role,
        auth_user.id,
        context.clock.now(),
    )?;
    email_invite(
        log,
        context,
        &query_org,
        &query_invite,
        &secret,
        auth_user,
        name,
    )
    .await?;

    Ok(JsonAuthAck {
        email: query_invite.email,
    })
}

/// Whether `email` belongs to a member of the organization.
fn is_member(
    conn: &mut DbConnection,
    organization_id: OrganizationId,
    email: &Email,
) -> Result<bool, HttpError> {
    diesel::select(diesel::dsl::exists(
        schema::organization_role::table
            .inner_join(schema::user::table)
            .filter(schema::organization_role::organization_id.eq(organization_id))
            .filter(schema::user::email.eq(email)),
    ))
    .get_result(conn)
    .map_err(|e| {
        issue_error(
            "Failed to look up a member",
            "Failed to look up whether an email belongs to a member:",
            e,
        )
    })
}

#[derive(Deserialize, JsonSchema)]
pub struct OrgMemberParams {
    /// The slug or UUID for an organization.
    pub organization: OrganizationResourceId,
    /// The slug or UUID for an organization member.
    pub user: UserResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/members/{user}",
    tags = ["organizations", "members"]
}]
pub async fn org_member_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrgMemberParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into(), Delete.into()]))
}

/// View an organization member
///
/// View a member of an organization.
/// The user must have `view_role` permissions for the organization.
#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/members/{user}",
    tags = ["organizations", "members"]
}]
pub async fn org_member_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrgMemberParams>,
) -> Result<ResponseOk<JsonMember>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: OrgMemberParams,
    auth_user: &AuthUser,
) -> Result<JsonMember, HttpError> {
    let query_organization = QueryOrganization::is_allowed_resource_id(
        auth_conn!(context),
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::ViewRole,
    )?;
    let query_user = QueryUser::from_resource_id(auth_conn!(context), &path_params.user)?;

    json_member(auth_conn!(context), query_user.id, query_organization.id)
}

/// Update an organization member
///
/// Update the role for a member of an organization.
/// The user must have `edit_role` permissions for the organization to change a role,
/// and `view_role` permissions to update nothing.
/// Changing a role requires a user session signed in within the last 10 minutes:
/// neither a user API key nor a deprecated user API token can grant a role.
#[endpoint {
    method = PATCH,
    path =  "/v0/organizations/{organization}/members/{user}",
    tags = ["organizations", "members"]
}]
pub async fn org_member_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrgMemberParams>,
    body: TypedBody<JsonUpdateMember>,
) -> Result<ResponseOk<JsonMember>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json_update = body.into_inner();
    if json_update.role.is_some() {
        // Checked only after authentication: an invalid credential still gets the opaque `401`.
        if !auth_user.is_session() {
            return Err(forbidden_error(
                "Only a user session can change a member's role.",
            ));
        }
        auth_user.check_fresh_sign_in(rqctx.context()).await?;
    }
    let json = patch_inner(
        rqctx.context(),
        path_params.into_inner(),
        json_update,
        &auth_user,
    )
    .await?;
    Ok(Patch::auth_response_ok(json))
}

async fn patch_inner(
    context: &ApiContext,
    path_params: OrgMemberParams,
    json_update: JsonUpdateMember,
    auth_user: &AuthUser,
) -> Result<JsonMember, HttpError> {
    let permission = if json_update.role.is_some() {
        Permission::EditRole
    } else {
        Permission::ViewRole
    };
    let query_organization = QueryOrganization::is_allowed_resource_id(
        auth_conn!(context),
        &context.rbac,
        &path_params.organization,
        auth_user,
        permission,
    )?;
    let query_user = QueryUser::from_resource_id(auth_conn!(context), &path_params.user)?;

    if let Some(role) = json_update.role {
        diesel::update(
            schema::organization_role::table
                .filter(schema::organization_role::user_id.eq(query_user.id))
                .filter(schema::organization_role::organization_id.eq(query_organization.id)),
        )
        .set(schema::organization_role::role.eq(role.to_string()))
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(
            OrganizationRole,
            (&query_user, &query_organization, role)
        ))?;
    }

    json_member(auth_conn!(context), query_user.id, query_organization.id)
}

/// Remove an organization member
///
/// Remove a member member of an organization.
/// The user must have `delete_role` permissions for the organization.
#[endpoint {
    method = DELETE,
    path =  "/v0/organizations/{organization}/members/{user}",
    tags = ["organizations", "members"]
}]
pub async fn org_member_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrgMemberParams>,
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: OrgMemberParams,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    let query_organization = QueryOrganization::is_allowed_resource_id(
        auth_conn!(context),
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::DeleteRole,
    )?;
    let query_user = QueryUser::from_resource_id(auth_conn!(context), &path_params.user)?;

    diesel::delete(
        schema::organization_role::table
            .filter(schema::organization_role::user_id.eq(query_user.id))
            .filter(schema::organization_role::organization_id.eq(query_organization.id)),
    )
    .execute(write_conn!(context))
    .map_err(resource_conflict_err!(
        OrganizationRole,
        (&query_user, query_organization)
    ))?;

    Ok(())
}

fn json_member(
    conn: &mut DbConnection,
    user_id: UserId,
    organization_id: OrganizationId,
) -> Result<JsonMember, HttpError> {
    Ok(schema::user::table
        .inner_join(schema::organization_role::table)
        .filter(schema::organization_role::user_id.eq(user_id))
        .filter(schema::organization_role::organization_id.eq(organization_id))
        .select((
            schema::user::uuid,
            schema::user::name,
            schema::user::slug,
            schema::user::email,
            schema::organization_role::role,
            schema::organization_role::created,
            schema::organization_role::modified,
        ))
        .first::<QueryMember>(conn)
        .map_err(resource_not_found_err!(
            OrganizationRole,
            (user_id, organization_id)
        ))?
        .into_json())
}
