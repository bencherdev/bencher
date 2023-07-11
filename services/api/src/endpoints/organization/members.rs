use std::str::FromStr;

use bencher_json::{
    organization::member::{JsonNewMember, JsonUpdateMember},
    JsonDirection, JsonEmpty, JsonMember, JsonPagination, ResourceId, UserName,
};
use bencher_rbac::organization::Permission;
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    context::{ApiContext, Body, ButtonBody, DbConnection, Message},
    endpoints::{
        endpoint::{response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::organization::{member::QueryMember, QueryOrganization},
    model::user::{auth::AuthUser, QueryUser},
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::into_json,
    },
    ApiError,
};

use super::Resource;

const MEMBER_RESOURCE: Resource = Resource::Member;

// TODO Custom max TTL
pub const INVITE_TOKEN_TTL: u32 = u32::MAX;

#[derive(Deserialize, JsonSchema)]
pub struct OrgMembersParams {
    pub organization: ResourceId,
}

pub type OrgMembersQuery = JsonPagination<OrgMembersSort, OrgMembersQueryParams>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrgMembersSort {
    #[default]
    Name,
}

#[derive(Deserialize, JsonSchema)]
pub struct OrgMembersQueryParams {
    pub name: Option<UserName>,
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
    _query_params: Query<OrgMembersQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/members",
    tags = ["organizations", "members"]
}]
pub async fn org_members_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OrgMembersParams>,
    query_params: Query<OrgMembersQuery>,
) -> Result<ResponseOk<Vec<JsonMember>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(MEMBER_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        &auth_user,
        path_params.into_inner(),
        query_params.into_inner(),
        endpoint,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: &AuthUser,
    path_params: OrgMembersParams,
    query_params: OrgMembersQuery,
    endpoint: Endpoint,
) -> Result<Vec<JsonMember>, ApiError> {
    let conn = &mut *context.conn().await;

    let query_organization = QueryOrganization::is_allowed_resource_id(
        conn,
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::ViewRole,
    )?;

    let mut query = schema::user::table
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
            schema::organization_role::created,
            schema::organization_role::modified,
        ))
        .into_boxed();

    if let Some(name) = query_params.query.name.as_ref() {
        query = query.filter(schema::user::name.eq(name.as_ref()));
    }

    query = match query_params.order() {
        OrgMembersSort::Name => match query_params.direction {
            Some(JsonDirection::Asc) | None => {
                query.order((schema::user::name.asc(), schema::user::slug.asc()))
            },
            Some(JsonDirection::Desc) => {
                query.order((schema::user::name.desc(), schema::user::slug.desc()))
            },
        },
    };

    Ok(query
        .offset(query_params.offset())
        .limit(query_params.limit())
        .load::<QueryMember>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(into_json!(endpoint))
        .collect())
}

#[endpoint {
    method = POST,
    path =  "/v0/organizations/{organization}/members",
    tags = ["organizations", "members"]
}]
pub async fn org_member_post(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OrgMembersParams>,
    body: TypedBody<JsonNewMember>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(MEMBER_RESOURCE, Method::Post);

    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &ApiContext,
    path_params: OrgMembersParams,
    mut json_new_member: JsonNewMember,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, ApiError> {
    let conn = &mut *context.conn().await;

    // Get the organization
    let query_org = QueryOrganization::from_resource_id(conn, &path_params.organization)?;

    // Check to see if user has permission to create a project within the organization
    context
        .rbac
        .is_allowed_organization(auth_user, Permission::CreateRole, &query_org)?;

    let email = json_new_member.email.clone();
    // If a user already exists for the email then direct them to login.
    // Otherwise, direct them to signup.
    let (name, route): (Option<String>, &str) = if let Ok(name) = schema::user::table
        .filter(schema::user::email.eq(email.as_ref()))
        .select(schema::user::name)
        .first(conn)
    {
        (Some(name), "/auth/login")
    } else {
        (json_new_member.name.take().map(Into::into), "/auth/signup")
    };

    // Get the requester user name and email for the message
    let (user_name, user_email) = schema::user::table
        .filter(schema::user::id.eq(auth_user.id))
        .select((schema::user::name, schema::user::email))
        .first::<(String, String)>(conn)
        .map_err(api_error!())?;

    // Create an invite token
    let token = context.secret_key.new_invite(
        json_new_member.email,
        INVITE_TOKEN_TTL,
        Uuid::from_str(&query_org.uuid).map_err(api_error!())?,
        json_new_member.role,
    )?;
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
            .join("/console/settings/email")
            .map(Into::into)
            .unwrap_or_default(),
    }));
    let message = Message {
        to_name: None,
        to_email: email.to_string(),
        subject: Some(format!("Invitation to join {org_name}")),
        body: Some(body),
    };
    context.messenger.send(message).await;

    Ok(JsonEmpty::default())
}

#[derive(Deserialize, JsonSchema)]
pub struct OrgMemberParams {
    pub organization: ResourceId,
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
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/members/{user}",
    tags = ["organizations", "members"]
}]
pub async fn org_member_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OrgMemberParams>,
) -> Result<ResponseOk<JsonMember>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(MEMBER_RESOURCE, Method::GetOne);

    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: OrgMemberParams,
    auth_user: &AuthUser,
) -> Result<JsonMember, ApiError> {
    let conn = &mut *context.conn().await;

    let query_organization = QueryOrganization::is_allowed_resource_id(
        conn,
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::ViewRole,
    )?;
    let query_user = QueryUser::from_resource_id(conn, &path_params.user)?;

    json_member(conn, query_user.id, query_organization.id)
}

#[endpoint {
    method = PATCH,
    path =  "/v0/organizations/{organization}/members/{user}",
    tags = ["organizations", "members"]
}]
pub async fn org_member_patch(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OrgMemberParams>,
    body: TypedBody<JsonUpdateMember>,
) -> Result<ResponseAccepted<JsonMember>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(MEMBER_RESOURCE, Method::Patch);

    let json = patch_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn patch_inner(
    context: &ApiContext,
    path_params: OrgMemberParams,
    json_update: JsonUpdateMember,
    auth_user: &AuthUser,
) -> Result<JsonMember, ApiError> {
    let conn = &mut *context.conn().await;

    let query_organization = QueryOrganization::from_resource_id(conn, &path_params.organization)?;
    let query_user = QueryUser::from_resource_id(conn, &path_params.user)?;

    if let Some(role) = json_update.role {
        // Verify that the user is allowed to update member role
        QueryOrganization::is_allowed_id(
            conn,
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
        .execute(conn)
        .map_err(api_error!())?;
    }

    json_member(conn, query_user.id, query_organization.id)
}

#[endpoint {
    method = DELETE,
    path =  "/v0/organizations/{organization}/members/{user}",
    tags = ["organizations", "members"]
}]
pub async fn org_member_delete(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OrgMemberParams>,
) -> Result<ResponseAccepted<JsonMember>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(MEMBER_RESOURCE, Method::Delete);

    let json = delete_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn delete_inner(
    context: &ApiContext,
    path_params: OrgMemberParams,
    auth_user: &AuthUser,
) -> Result<JsonMember, ApiError> {
    let conn = &mut *context.conn().await;

    let query_organization = QueryOrganization::is_allowed_resource_id(
        conn,
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::DeleteRole,
    )?;
    let query_user = QueryUser::from_resource_id(conn, &path_params.user)?;

    let json_member = json_member(conn, query_user.id, query_organization.id)?;

    diesel::delete(
        schema::organization_role::table
            .filter(schema::organization_role::user_id.eq(query_user.id))
            .filter(schema::organization_role::organization_id.eq(query_organization.id)),
    )
    .execute(conn)
    .map_err(api_error!())?;

    Ok(json_member)
}

fn json_member(
    conn: &mut DbConnection,
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
            schema::organization_role::created,
            schema::organization_role::modified,
        ))
        .first::<QueryMember>(conn)
        .map_err(api_error!())?
        .into_json()
}
