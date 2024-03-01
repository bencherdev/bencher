use bencher_json::{
    organization::member::{JsonNewMember, JsonUpdateMember},
    JsonAuthAck, JsonDirection, JsonMember, JsonMembers, JsonPagination, ResourceId, UserName,
};
use bencher_rbac::organization::Permission;
use diesel::{
    BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl, TextExpressionMethods,
};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use http::StatusCode;
use schemars::JsonSchema;
use serde::Deserialize;
use slog::Logger;

use crate::{
    conn_lock,
    context::{ApiContext, Body, ButtonBody, DbConnection, Message},
    endpoints::{
        endpoint::{
            CorsResponse, Delete, Get, Patch, Post, ResponseAccepted, ResponseDeleted, ResponseOk,
        },
        Endpoint,
    },
    error::{forbidden_error, issue_error, resource_conflict_err, resource_not_found_err},
    model::user::{
        auth::{AuthUser, BearerToken},
        QueryUser,
    },
    model::{
        organization::{member::QueryMember, OrganizationId, QueryOrganization},
        user::UserId,
    },
    schema,
    util::search::Search,
};

// TODO Custom max TTL
pub const INVITE_TOKEN_TTL: u32 = u32::MAX;

#[derive(Deserialize, JsonSchema)]
pub struct OrgMembersParams {
    /// The slug or UUID for an organization.
    pub organization: ResourceId,
}

pub type OrgMembersPagination = JsonPagination<OrgMembersSort>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrgMembersSort {
    /// Sort by user name.
    #[default]
    Name,
}

#[derive(Deserialize, JsonSchema)]
pub struct OrgMembersQuery {
    /// Filter by user name, exact match.
    pub name: Option<UserName>,
    /// Search by user name, slug, or UUID.
    pub search: Option<Search>,
}

#[allow(clippy::unused_async)]
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
    let json = get_ls_inner(
        rqctx.context(),
        &auth_user,
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params.into_inner(),
    )
    .await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: &AuthUser,
    path_params: OrgMembersParams,
    pagination_params: OrgMembersPagination,
    query_params: OrgMembersQuery,
) -> Result<JsonMembers, HttpError> {
    let query_organization = QueryOrganization::is_allowed_resource_id(
        conn_lock!(context),
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::ViewRole,
    )?;

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

    query = match pagination_params.order() {
        OrgMembersSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => {
                query.order((schema::user::name.asc(), schema::user::slug.asc()))
            },
            Some(JsonDirection::Desc) => {
                query.order((schema::user::name.desc(), schema::user::slug.desc()))
            },
        },
    };

    Ok(query
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryMember>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            OrganizationRole,
            query_organization
        ))?
        .into_iter()
        .map(QueryMember::into_json)
        .collect())
}

/// Invite a user to an organization
///
/// Invite another user to become a member of an organization.
/// The user must have `create_role` permissions for the organization.
/// The invitee is sent an email with a link to accept the invitation, and
/// they are not added to the organization until they accept the invitation.
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
    mut json_new_member: JsonNewMember,
    auth_user: &AuthUser,
) -> Result<JsonAuthAck, HttpError> {
    // Get the organization
    let query_org =
        QueryOrganization::from_resource_id(conn_lock!(context), &path_params.organization)?;

    // Check to see if user has permission to create a project within the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::CreateRole, &query_org)
        .map_err(forbidden_error)?;

    let email = json_new_member.email.clone();
    // If a user already exists for the email then direct them to login.
    // Otherwise, direct them to signup.
    let (name, route): (Option<String>, &str) =
        if let Ok(user) = QueryUser::get_with_email(conn_lock!(context), &email) {
            (Some(user.name.into()), "/auth/login")
        } else {
            (json_new_member.name.take().map(Into::into), "/auth/signup")
        };

    // Create an invite token
    let token = context
        .token_key
        .new_invite(
            json_new_member.email,
            INVITE_TOKEN_TTL,
            query_org.uuid,
            json_new_member.role,
        )
        .map_err(|e| {
            issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create new invite token",
                "Failed to create new invite token.",
                e,
            )
        })?;
    let token_string = token.to_string();

    let org_name = &query_org.name;
    let org_role = json_new_member.role;
    let body = Body::Button(Box::new(ButtonBody {
        title: format!("Invitation to join {org_name}"),
        preheader: "Click the provided link to join.".into(),
        greeting: if let Some(name) = name {
            format!("Ahoy {name}!") } else { "Ahoy!".into() },
        pre_body: format!(
            "Please, click the button below or use the provided code to accept the invitation from {user_name} ({user_email}) to join {org_name} as a {org_role} on Bencher.",
            user_name = auth_user.user.name,
            user_email = auth_user.user.email,
        ),
        button_text: format!("Join {org_name}"),
        button_url: context
            .endpoint
            .clone()
            .join(route)
            .map(|mut url| {
                url.query_pairs_mut().append_pair("invite", &token_string);
                url.into()
            })
            .unwrap_or_default(),
        clipboard_text: "Invite Code".into(),
        clipboard_target: token_string,
        post_body: String::new(),
        closing: "See you soon,".into(),
        signature: "The Bencher Team".into(),
        settings_url: context
            .endpoint
            .clone()
            .join("/help")
            .map(Into::into)
            .unwrap_or_default(),
    }));
    let message = Message {
        to_name: None,
        to_email: email.to_string(),
        subject: Some(format!("Invitation to join {org_name}")),
        body: Some(body),
    };
    context.messenger.send(log, message);

    Ok(JsonAuthAck { email })
}

#[derive(Deserialize, JsonSchema)]
pub struct OrgMemberParams {
    /// The slug or UUID for an organization.
    pub organization: ResourceId,
    /// The slug or UUID for an organization member.
    pub user: ResourceId,
}

#[allow(clippy::unused_async)]
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
        conn_lock!(context),
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::ViewRole,
    )?;
    let query_user = QueryUser::from_resource_id(conn_lock!(context), &path_params.user)?;

    json_member(conn_lock!(context), query_user.id, query_organization.id)
}

/// Update an organization member
///
/// Update the role for a member of an organization.
/// The user must have `edit_role` permissions for the organization.
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
    path_params: OrgMemberParams,
    json_update: JsonUpdateMember,
    auth_user: &AuthUser,
) -> Result<JsonMember, HttpError> {
    let query_organization =
        QueryOrganization::from_resource_id(conn_lock!(context), &path_params.organization)?;
    let query_user = QueryUser::from_resource_id(conn_lock!(context), &path_params.user)?;

    if let Some(role) = json_update.role {
        // Verify that the user is allowed to update member role
        QueryOrganization::is_allowed_id(
            conn_lock!(context),
            &context.rbac,
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
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(
            OrganizationRole,
            (&query_user, &query_organization, role)
        ))?;
    }

    json_member(conn_lock!(context), query_user.id, query_organization.id)
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
        conn_lock!(context),
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::DeleteRole,
    )?;
    let query_user = QueryUser::from_resource_id(conn_lock!(context), &path_params.user)?;

    diesel::delete(
        schema::organization_role::table
            .filter(schema::organization_role::user_id.eq(query_user.id))
            .filter(schema::organization_role::organization_id.eq(query_organization.id)),
    )
    .execute(conn_lock!(context))
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
