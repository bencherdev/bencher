use bencher_json::{JsonEmpty, JsonSignup};
use bencher_rbac::organization::Role;
use chrono::Utc;
use diesel::dsl::count;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use slog::Logger;

use crate::context::NewUserBody;
use crate::endpoints::endpoint::pub_response_accepted;
use crate::endpoints::endpoint::CorsResponse;
use crate::endpoints::endpoint::{Endpoint, ResponseAccepted};
use crate::model::organization::{
    organization_role::InsertOrganizationRole, InsertOrganization, QueryOrganization,
};
use crate::model::user::QueryUser;
use crate::ApiError;
use crate::{
    context::{ApiContext, Body, ButtonBody, Message},
    model::user::InsertUser,
    schema,
};

use super::AUTH_TOKEN_TTL;
use super::TOKEN_ARG;

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/signup",
    tags = ["auth"]
}]
pub async fn auth_signup_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Endpoint::Post]))
}

#[endpoint {
    method = POST,
    path =  "/v0/auth/signup",
    tags = ["auth"]
}]
pub async fn auth_signup_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonSignup>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let endpoint = Endpoint::Post;

    let json = post_inner(&rqctx.log, rqctx.context(), body.into_inner())
        .await
        .map_err(|e| {
            if let ApiError::HttpError(e) = e {
                e
            } else {
                endpoint.err(e).into()
            }
        })?;

    pub_response_accepted!(endpoint, json)
}

#[allow(clippy::too_many_lines)]
async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    mut json_signup: JsonSignup,
) -> Result<JsonEmpty, ApiError> {
    let conn = &mut *context.conn().await;

    #[cfg(feature = "plus")]
    let plan = json_signup.plan.unwrap_or_default();

    let invite = json_signup.invite.take();
    let email = json_signup.email.clone();
    let mut insert_user = InsertUser::from_json(conn, json_signup)?;

    let count = schema::user::table
        .select(count(schema::user::id))
        .first::<i64>(conn)
        .map_err(ApiError::from)?;
    // The first user to signup is admin
    if count == 0 {
        insert_user.admin = true;
    }

    // Insert user
    diesel::insert_into(schema::user::table)
        .values(&insert_user)
        .execute(conn)
        .map_err(ApiError::from)?;
    let user_id = QueryUser::get_id(conn, insert_user.uuid)?;

    let insert_org_role = if let Some(invite) = &invite {
        InsertOrganizationRole::from_jwt(conn, &context.secret_key, invite, user_id)?
    } else {
        // Create an organization for the user
        let insert_org = InsertOrganization::from_user(&insert_user);
        diesel::insert_into(schema::organization::table)
            .values(&insert_org)
            .execute(conn)
            .map_err(ApiError::from)?;
        let organization_id = QueryOrganization::get_id(conn, insert_org.uuid)?;

        let timestamp = Utc::now().timestamp();
        // Connect the user to the organization as a `Leader`
        InsertOrganizationRole {
            user_id,
            organization_id,
            role: Role::Leader.to_string(),
            created: timestamp,
            modified: timestamp,
        }
    };

    diesel::insert_into(schema::organization_role::table)
        .values(&insert_org_role)
        .execute(conn)
        .map_err(ApiError::from)?;

    let token = context.secret_key.new_auth(email, AUTH_TOKEN_TTL)?;

    let token_string = token.to_string();
    let body = Body::Button(Box::new(ButtonBody {
        title: "Confirm Bencher Signup".into(),
        preheader: "Click the provided link to signup.".into(),
        greeting: format!("Ahoy {},", insert_user.name),
        pre_body: "Please, click the button below or use the provided code to signup for Bencher."
            .into(),
        button_text: "Confirm Email".into(),
        button_url: context
            .endpoint
            .clone()
            .join("/auth/confirm")
            .map(|mut url| {
                #[cfg(feature = "plus")]
                url.query_pairs_mut()
                    .append_pair(super::PLAN_ARG, plan.as_ref());
                url.query_pairs_mut().append_pair(TOKEN_ARG, &token_string);
                url.into()
            })
            .unwrap_or_default(),
        clipboard_text: "Confirmation Code".into(),
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
        to_name: Some(insert_user.name.clone()),
        to_email: insert_user.email.clone(),
        subject: Some("Confirm Bencher Signup".into()),
        body: Some(body),
    };
    context.messenger.send(log, message);

    if !insert_user.admin {
        let admins = QueryUser::get_admins(conn)?;
        for admin in admins {
            let message = Message {
                to_name: Some(admin.name.clone()),
                to_email: admin.email.clone(),
                subject: Some("🐰 New Bencher User".into()),
                body: Some(Body::NewUser(NewUserBody {
                    admin: admin.name.clone(),
                    endpoint: context.endpoint.clone(),
                    name: insert_user.name.clone(),
                    email: insert_user.email.clone(),
                    invited: invite.is_some(),
                })),
            };
            context.messenger.send(log, message);
        }
    }

    Ok(JsonEmpty::default())
}
