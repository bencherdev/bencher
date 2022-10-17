use std::sync::Arc;

use bencher_json::{jwt::JsonWebToken, JsonEmpty, JsonSignup};
use bencher_rbac::organization::Role;
use diesel::dsl::count;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};

use crate::endpoints::endpoint::pub_response_accepted;
use crate::endpoints::endpoint::ResponseAccepted;
use crate::endpoints::Endpoint;
use crate::endpoints::Method;
use crate::error::api_error;
use crate::model::organization::InsertOrganization;
use crate::model::organization::QueryOrganization;
use crate::model::user::organization::InsertOrganizationRole;
use crate::model::user::QueryUser;
use crate::util::context::{Body, ButtonBody, Message};
use crate::util::cors::CorsResponse;
use crate::ApiError;
use crate::{
    model::user::InsertUser,
    schema,
    util::{cors::get_cors, Context},
};

use super::Resource;

const SIGNUP_RESOURCE: Resource = Resource::Signup;

#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/signup",
    tags = ["auth"]
}]
pub async fn options(_rqctx: Arc<RequestContext<Context>>) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path =  "/v0/auth/signup",
    tags = ["auth"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonSignup>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let endpoint = Endpoint::new(SIGNUP_RESOURCE, Method::Post);

    let json = post_inner(rqctx.context(), body.into_inner())
        .await
        .map_err(|e| endpoint.err(e))?;

    pub_response_accepted!(endpoint, json)
}

async fn post_inner(context: &Context, mut json_signup: JsonSignup) -> Result<JsonEmpty, ApiError> {
    let api_context = &mut *context.lock().await;
    let conn = &mut api_context.db_conn;

    let invite = json_signup.invite.take();
    let mut insert_user = InsertUser::from_json(conn, json_signup)?;

    let count = schema::user::table
        .select(count(schema::user::id))
        .first::<i64>(conn)
        .map_err(api_error!())?;
    // The first user to signup is admin
    if count == 0 {
        insert_user.admin = true;
    }

    // Insert user
    diesel::insert_into(schema::user::table)
        .values(&insert_user)
        .execute(conn)
        .map_err(api_error!())?;
    let user_id = QueryUser::get_id(conn, &insert_user.uuid)?;

    let insert_org_role = if let Some(invite) = &invite {
        InsertOrganizationRole::from_jwt(conn, invite, &api_context.secret_key, user_id)?
    } else {
        // Create an organization for the user
        let insert_org = InsertOrganization::from_user(&insert_user);
        diesel::insert_into(schema::organization::table)
            .values(&insert_org)
            .execute(conn)
            .map_err(api_error!())?;
        let organization_id = QueryOrganization::get_id(conn, &insert_org.uuid)?;

        // Connect the user to the organization as a `Leader`
        InsertOrganizationRole {
            user_id,
            organization_id,
            role: Role::Leader.to_string(),
        }
    };

    diesel::insert_into(schema::organization_role::table)
        .values(&insert_org_role)
        .execute(conn)
        .map_err(api_error!())?;

    let token = JsonWebToken::new_auth(&api_context.secret_key.encoding, insert_user.email.clone())
        .map_err(api_error!())?;

    let token_string = token.to_string();
    let body = Body::Button(ButtonBody {
        title: "Confirm Bencher Signup".into(),
        preheader: "Click the provided link to signup.".into(),
        greeting: format!("Ahoy {},", insert_user.name),
        pre_body: format!("Please, click the button below or use the provided code to signup."),
        pre_code: "".into(),
        button_text: "Confirm Email".into(),
        button_url: api_context
            .url
            .clone()
            .join("/auth/confirm")
            .map(|mut url| {
                url.query_pairs_mut().append_pair("token", &token_string);
                url.into()
            })
            .unwrap_or_default(),
        post_body: "Code: ".into(),
        post_code: token_string,
        closing: "See you soon,".into(),
        signature: "The Bencher Team".into(),
        settings_url: api_context
            .url
            .clone()
            .join("/console/settings/email")
            .map(Into::into)
            .unwrap_or_default(),
    });
    let message = Message {
        to_name: Some(insert_user.name),
        to_email: insert_user.email,
        subject: Some("Confirm Bencher Signup".into()),
        body: Some(body),
    };
    api_context.messenger.send(message).await;

    Ok(JsonEmpty::default())
}
