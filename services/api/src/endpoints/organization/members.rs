use std::{str::FromStr, sync::Arc};

use bencher_json::{
    organization::member::{JsonNewMember, JsonUpdateMember},
    system::jwt::JsonWebToken,
    JsonEmpty, JsonMember, ResourceId,
};
use bencher_rbac::organization::Permission;
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl, SqliteConnection};
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
    model::organization::{member::QueryMember, QueryOrganization},
    model::user::{auth::AuthUser, validate_email, QueryUser},
    schema,
    util::{
        context::{Body, ButtonBody, Message},
        cors::{get_cors, CorsResponse},
        error::into_json,
        Context,
    },
    ApiError,
};

use super::Resource;

const MEMBER_RESOURCE: Resource = Resource::Member;

#[derive(Deserialize, JsonSchema)]
pub struct GetDirParams {
    pub organization: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/members",
    tags = ["organizations", "members"]
}]
pub async fn dir_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetDirParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/members",
    tags = ["organizations", "members"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetDirParams>,
) -> Result<ResponseOk<Vec<JsonMember>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(MEMBER_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        &auth_user,
        path_params.into_inner(),
        endpoint,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_ls_inner(
    context: &Context,
    auth_user: &AuthUser,
    path_params: GetDirParams,
    endpoint: Endpoint,
) -> Result<Vec<JsonMember>, ApiError> {
    let api_context = &mut *context.lock().await;
    let query_organization = QueryOrganization::is_allowed_resource_id(
        api_context,
        &path_params.organization,
        auth_user,
        Permission::ViewRole,
    )?;
    let conn = &mut api_context.database;

    Ok(schema::user::table
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
        ))
        .order(schema::user::email)
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
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetDirParams>,
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
    context: &Context,
    path_params: GetDirParams,
    mut json_new_member: JsonNewMember,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, ApiError> {
    let api_context = &mut *context.lock().await;
    let conn = &mut api_context.database;

    // Get the organization
    let query_org = QueryOrganization::from_resource_id(conn, &path_params.organization)?;

    // Check to see if user has permission to create a project within the organization
    api_context
        .rbac
        .is_allowed_organization(auth_user, Permission::CreateRole, &query_org)?;

    let email = json_new_member.email.clone();
    // If a user already exists for the email then direct them to login.
    // Otherwise, direct them to signup.
    let (name, route) = if let Ok(name) = schema::user::table
        .filter(schema::user::email.eq(&email))
        .select(schema::user::name)
        .first(conn)
    {
        (Some(name), "/auth/login")
    } else {
        validate_email(&email)?;
        (json_new_member.name.take(), "/auth/signup")
    };

    // Get the requester user name and email for the message
    let (user_name, user_email) = schema::user::table
        .filter(schema::user::id.eq(auth_user.id))
        .select((schema::user::name, schema::user::email))
        .first::<(String, String)>(conn)
        .map_err(api_error!())?;

    // Create an invite token
    let token = JsonWebToken::new_invite(
        &api_context.secret_key.encoding,
        json_new_member.email,
        Uuid::from_str(&query_org.uuid).map_err(api_error!())?,
        json_new_member.role,
    )
    .map_err(api_error!())?;
    let token_string = token.to_string();

    let org_name = &query_org.name;
    let org_role = json_new_member.role;
    let body = Body::Button(ButtonBody {
        title: format!("Invitation to join {org_name}"),
        preheader: "Click the provided link to join.".into(),
        greeting: if let Some(name) = name {
            format!("Ahoy {name}!") } else { "Ahoy!".into() },
        pre_body: format!(
            "Please, click the button below or use the provided code to accept the invitation from {user_name} ({user_email}) to join {org_name} as a {org_role} on Bencher.",
        ),
        pre_code: "".into(),
        button_text: format!("Join {org_name}"),
        button_url: api_context
            .endpoint
            .clone()
            .join(route)
            .map(|mut url| {
                url.query_pairs_mut().append_pair("invite", &token_string);
                url.into()
            })
            .unwrap_or_default(),
        post_body: "Code: ".into(),
        post_code: token_string,
        closing: "See you soon,".into(),
        signature: "The Bencher Team".into(),
        settings_url: api_context
            .endpoint
            .clone()
            .join("/console/settings/email")
            .map(Into::into)
            .unwrap_or_default(),
    });
    let message = Message {
        to_name: None,
        to_email: email.to_string(),
        subject: Some(format!("Invitation to join {org_name}")),
        body: Some(body),
    };
    api_context.messenger.send(message).await;

    Ok(JsonEmpty::default())
}

#[derive(Deserialize, JsonSchema)]
pub struct GetOneParams {
    pub organization: ResourceId,
    pub user: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/members/{user}",
    tags = ["organizations", "members"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetOneParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/members/{user}",
    tags = ["organizations", "members"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetOneParams>,
) -> Result<ResponseOk<JsonMember>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(MEMBER_RESOURCE, Method::GetOne);

    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_one_inner(
    context: &Context,
    path_params: GetOneParams,
    auth_user: &AuthUser,
) -> Result<JsonMember, ApiError> {
    let api_context = &mut *context.lock().await;
    let query_organization = QueryOrganization::is_allowed_resource_id(
        api_context,
        &path_params.organization,
        auth_user,
        Permission::ViewRole,
    )?;
    let conn = &mut api_context.database;
    let query_user = QueryUser::from_resource_id(conn, &path_params.user)?;

    json_member(conn, query_user.id, query_organization.id)
}

#[endpoint {
    method = PATCH,
    path =  "/v0/organizations/{organization}/members/{user}",
    tags = ["organizations", "members"]
}]
pub async fn patch(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetOneParams>,
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
    context: &Context,
    path_params: GetOneParams,
    json_update: JsonUpdateMember,
    auth_user: &AuthUser,
) -> Result<JsonMember, ApiError> {
    let api_context = &mut *context.lock().await;

    let query_organization =
        QueryOrganization::from_resource_id(&mut api_context.database, &path_params.organization)?;
    let query_user = QueryUser::from_resource_id(&mut api_context.database, &path_params.user)?;

    if let Some(role) = json_update.role {
        // Verify that the user is allowed to update member role
        QueryOrganization::is_allowed_id(
            api_context,
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
        .execute(&mut api_context.database)
        .map_err(api_error!())?;
    }

    json_member(
        &mut api_context.database,
        query_user.id,
        query_organization.id,
    )
}

#[endpoint {
    method = DELETE,
    path =  "/v0/organizations/{organization}/members/{user}",
    tags = ["organizations", "members"]
}]
pub async fn delete(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetOneParams>,
) -> Result<ResponseAccepted<JsonMember>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(MEMBER_RESOURCE, Method::Patch);

    let json = delete_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn delete_inner(
    context: &Context,
    path_params: GetOneParams,
    auth_user: &AuthUser,
) -> Result<JsonMember, ApiError> {
    let api_context = &mut *context.lock().await;
    let query_organization = QueryOrganization::is_allowed_resource_id(
        api_context,
        &path_params.organization,
        auth_user,
        Permission::DeleteRole,
    )?;
    let conn = &mut api_context.database;
    let query_user = QueryUser::from_resource_id(conn, &path_params.user)?;

    let json_member = json_member(conn, query_user.id, query_organization.id)?;

    diesel::delete(
        schema::organization_role::table
            .filter(schema::organization_role::user_id.eq(query_user.id))
            .filter(schema::organization_role::organization_id.eq(query_organization.id)),
    )
    .execute(&mut api_context.database)
    .map_err(api_error!())?;

    Ok(json_member)
}

fn json_member(
    conn: &mut SqliteConnection,
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
        ))
        .order(schema::user::email)
        .first::<QueryMember>(conn)
        .map_err(api_error!())?
        .into_json()
}
