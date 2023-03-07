use bencher_json::{JsonEmpty, JsonLogin};

use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};

use crate::endpoints::endpoint::pub_response_accepted;
use crate::endpoints::endpoint::ResponseAccepted;
use crate::endpoints::Endpoint;
use crate::endpoints::Method;
use crate::error::api_error;

use crate::{
    context::{ApiContext, Body, ButtonBody, Message},
    model::organization::organization_role::InsertOrganizationRole,
    model::user::QueryUser,
    schema,
    util::cors::{get_cors, CorsResponse},
    ApiError,
};

use super::Resource;
use super::AUTH_TOKEN_TTL;
use super::TOKEN_ARG;

const LOGIN_RESOURCE: Resource = Resource::Login;

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/login",
    tags = ["auth"]
}]
pub async fn options(_rqctx: RequestContext<ApiContext>) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = POST,
    path = "/v0/auth/login",
    tags = ["auth"]
}]
pub async fn post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonLogin>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let endpoint = Endpoint::new(LOGIN_RESOURCE, Method::Post);

    let json = post_inner(rqctx.context(), body.into_inner())
        .await
        .map_err(|e| endpoint.err(e))?;

    pub_response_accepted!(endpoint, json)
}

async fn post_inner(context: &ApiContext, json_login: JsonLogin) -> Result<JsonEmpty, ApiError> {
    let conn = &mut *context.conn().await;

    let query_user = schema::user::table
        .filter(schema::user::email.eq(json_login.email.as_ref()))
        .first::<QueryUser>(conn)
        .map_err(api_error!())?;

    // Check to see if the user account has been locked
    if query_user.locked {
        return Err(ApiError::Locked(query_user.id, query_user.email));
    }

    #[cfg(feature = "plus")]
    let plan = json_login.plan;

    if let Some(invite) = &json_login.invite {
        let insert_org_role =
            InsertOrganizationRole::from_jwt(conn, &context.secret_key, invite, query_user.id)?;

        diesel::insert_into(schema::organization_role::table)
            .values(&insert_org_role)
            .execute(conn)
            .map_err(api_error!())?;
    }

    let token = context
        .secret_key
        .new_auth(json_login.email.clone(), AUTH_TOKEN_TTL)?;

    let token_string = token.to_string();
    let body = Body::Button(Box::new(ButtonBody {
        title: "Confirm Bencher Login".into(),
        preheader: "Click the provided link to login.".into(),
        greeting: format!("Ahoy {},", query_user.name),
        pre_body: "Please, click the button below or use the provided code to login to Bencher."
            .into(),
        button_text: "Confirm Login".into(),
        button_url: context
            .endpoint
            .clone()
            .join("/auth/confirm")
            .map(|mut url| {
                #[cfg(feature = "plus")]
                if let Some(plan) = plan {
                    url.query_pairs_mut()
                        .append_pair(super::PLAN_ARG, plan.as_ref());
                }
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
            .join("/console/settings/email")
            .map(Into::into)
            .unwrap_or_default(),
    }));
    let message = Message {
        to_name: Some(query_user.name),
        to_email: query_user.email,
        subject: Some("Confirm Bencher Login".into()),
        body: Some(body),
    };
    context.messenger.send(message).await;

    Ok(JsonEmpty::default())
}
