use std::sync::Arc;

use bencher_json::{auth::Role, jwt::JsonWebToken, JsonEmpty, JsonLogin};
use bencher_rbac::organization::LEADER_ROLE;
use bencher_rbac::organization::MEMBER_ROLE;
use diesel::{QueryDsl, RunQueryDsl};
use dropshot::{
    endpoint, HttpError, HttpResponseAccepted, HttpResponseHeaders, RequestContext, TypedBody,
};
use tracing::info;

use crate::util::cors::CorsResponse;
use crate::{
    diesel::ExpressionMethods,
    model::organization::QueryOrganization,
    model::user::organization::InsertOrganizationRole,
    model::user::QueryUser,
    schema,
    util::{cors::get_cors, headers::CorsHeaders, http_error, map_http_error, Context},
};

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
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonEmpty>, CorsHeaders>, HttpError> {
    let json_login = body.into_inner();
    let context = &mut *rqctx.context().lock().await;

    let conn = &mut context.db;
    let query_user = schema::user::table
        .filter(schema::user::email.eq(&json_login.email))
        .first::<QueryUser>(conn)
        .map_err(map_http_error!("Failed to login user."))?;

    // Check to see if the user account has been locked
    if query_user.locked {
        return Err(http_error!("Failed to login user account locked."));
    }

    if let Some(invite) = json_login.invite {
        let token_data = invite
            .validate_invite(&context.key)
            .map_err(map_http_error!("Failed to login user."))?;
        let org_claims = token_data
            .claims
            .org()
            .ok_or_else(|| http_error!("Failed to login user."))?;

        // Connect the user to the organization with the given role
        let organization_id = QueryOrganization::get_id(conn, org_claims.uuid)?;
        let insert_org_role = InsertOrganizationRole {
            user_id: query_user.id,
            organization_id,
            // TODO better type casting
            role: match org_claims.role {
                Role::Member => MEMBER_ROLE,
                Role::Leader => LEADER_ROLE,
            }
            .into(),
        };

        diesel::insert_into(schema::organization_role::table)
            .values(&insert_org_role)
            .execute(conn)
            .map_err(map_http_error!("Failed to login user."))?;
    }

    let token = JsonWebToken::new_auth(&context.key, query_user.email)
        .map_err(map_http_error!("Failed to login user."))?;

    // TODO log this as trace if SMTP is configured
    info!("Confirm \"{}\" with: {token}", json_login.email);

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(JsonEmpty::default()),
        CorsHeaders::new_pub("POST".into()),
    ))
}
