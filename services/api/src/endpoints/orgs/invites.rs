use std::{str::FromStr, sync::Arc};

use bencher_json::{jwt::JsonWebToken, JsonEmpty, JsonInvite};
use bencher_rbac::organization::Permission;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use uuid::Uuid;

use crate::{
    endpoints::{
        endpoint::{response_accepted, ResponseAccepted},
        Endpoint, Method,
    },
    error::api_error,
    model::{
        organization::QueryOrganization,
        user::{auth::AuthUser, validate_email},
    },
    schema,
    util::{
        context::{Body, ButtonBody, Message},
        cors::{get_cors, CorsResponse},
        Context,
    },
    ApiError,
};

use super::Resource;

const INVITE_RESOURCE: Resource = Resource::Invite;

#[endpoint {
    method = OPTIONS,
    path =  "/v0/invites",
    tags = ["invites"]
}]
pub async fn options(_rqctx: Arc<RequestContext<Context>>) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path = "/v0/invites",
    tags = ["invites"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonInvite>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(INVITE_RESOURCE, Method::Post);

    let json = post_inner(rqctx.context(), body.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &Context,
    mut json_invite: JsonInvite,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, ApiError> {
    let api_context = &mut *context.lock().await;
    let conn = &mut api_context.database;

    // Get the organization
    let query_org = QueryOrganization::from_resource_id(conn, &json_invite.organization)?;

    // Check to see if user has permission to create a project within the organization
    api_context
        .rbac
        .is_allowed_organization(auth_user, Permission::CreateRole, &query_org)?;

    let email = json_invite.email.clone();
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
        (json_invite.name.take(), "/auth/signup")
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
        json_invite.email,
        Uuid::from_str(&query_org.uuid).map_err(api_error!())?,
        json_invite.role,
    )
    .map_err(api_error!())?;
    let token_string = token.to_string();

    let org_name = &query_org.name;
    let org_role = json_invite.role;
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
