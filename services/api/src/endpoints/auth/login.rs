use std::sync::Arc;

use bencher_json::{jwt::JsonWebToken, JsonEmpty, JsonLogin};

use diesel::{QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use tracing::info;

use crate::endpoints::endpoint::pub_response_accepted;
use crate::endpoints::endpoint::ResponseAccepted;
use crate::endpoints::Endpoint;
use crate::endpoints::Method;
use crate::error::api_error;

use crate::util::cors::CorsResponse;
use crate::ApiError;
use crate::{
    diesel::ExpressionMethods,
    model::user::organization::InsertOrganizationRole,
    model::user::QueryUser,
    schema,
    util::{cors::get_cors, Context},
};

use super::Resource;

const LOGIN_RESOURCE: Resource = Resource::Login;

#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/login",
    tags = ["auth"]
}]
pub async fn options(_rqctx: Arc<RequestContext<Context>>) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path = "/v0/auth/login",
    tags = ["auth"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonLogin>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let endpoint = Endpoint::new(LOGIN_RESOURCE, Method::Post);

    let json = post_inner(rqctx.context(), body.into_inner())
        .await
        .map_err(|e| endpoint.err(e))?;

    pub_response_accepted!(endpoint, json)
}

async fn post_inner(context: &Context, json_login: JsonLogin) -> Result<JsonEmpty, ApiError> {
    let api_context = &mut *context.lock().await;
    let conn = &mut api_context.db_conn;

    let query_user = schema::user::table
        .filter(schema::user::email.eq(&json_login.email))
        .first::<QueryUser>(conn)
        .map_err(api_error!())?;

    // Check to see if the user account has been locked
    if query_user.locked {
        return Err(ApiError::Locked(query_user.id, query_user.email));
    }

    if let Some(invite) = &json_login.invite {
        let insert_org_role =
            InsertOrganizationRole::from_jwt(conn, invite, &api_context.secret_key, query_user.id)?;

        diesel::insert_into(schema::organization_role::table)
            .values(&insert_org_role)
            .execute(conn)
            .map_err(api_error!())?;
    }

    let token = JsonWebToken::new_auth(&api_context.secret_key.encoding, query_user.email)
        .map_err(api_error!())?;

    // TODO log this as trace if SMTP is configured
    info!("Confirm \"{}\" with: {token}", json_login.email);

    Ok(JsonEmpty::default())
}
