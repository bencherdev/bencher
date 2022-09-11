use std::sync::Arc;

use bencher_json::{auth::Role, jwt::JsonWebToken, JsonEmpty, JsonSignup};
use bencher_rbac::organization::LEADER_ROLE;
use bencher_rbac::organization::MEMBER_ROLE;
use diesel::dsl::count;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use dropshot::{
    endpoint, HttpError, HttpResponseAccepted, HttpResponseHeaders, HttpResponseOk, RequestContext,
    TypedBody,
};
use tracing::info;

use crate::model::organization::InsertOrganization;
use crate::model::organization::QueryOrganization;
use crate::model::user::organization::InsertOrganizationRole;
use crate::model::user::QueryUser;
use crate::{
    model::user::InsertUser,
    schema,
    util::{cors::get_cors, headers::CorsHeaders, http_error, map_http_error, Context},
};

#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/signup",
    tags = ["auth"]
}]
pub async fn options(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
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
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonEmpty>, CorsHeaders>, HttpError> {
    let mut json_signup = body.into_inner();
    let context = &mut *rqctx.context().lock().await;

    let conn = &mut context.db;
    let invite = json_signup.invite.take();
    let mut insert_user = InsertUser::from_json(conn, json_signup)?;

    let count = schema::user::table
        .select(count(schema::user::id))
        .first::<i64>(conn)
        .map_err(map_http_error!("Failed to signup user."))?;
    // The first user to signup is admin
    if count == 0 {
        insert_user.admin = true;
    }

    // Insert user
    diesel::insert_into(schema::user::table)
        .values(&insert_user)
        .execute(conn)
        .map_err(map_http_error!("Failed to signup user."))?;
    let user_id = QueryUser::get_id(conn, &insert_user.uuid)?;

    let insert_org_role = if let Some(invite) = invite {
        let token_data = invite
            .validate_invite(&context.key)
            .map_err(map_http_error!("Failed to signup user."))?;
        let org_claims = token_data
            .claims
            .org()
            .ok_or_else(|| http_error!("Failed to signup user."))?;

        // Connect the user to the organization with the given role
        let organization_id = QueryOrganization::get_id(conn, org_claims.uuid)?;
        InsertOrganizationRole {
            user_id,
            organization_id,
            // TODO better type casting
            role: match org_claims.role {
                Role::Member => MEMBER_ROLE,
                Role::Leader => LEADER_ROLE,
            }
            .into(),
        }
    } else {
        // Create an organization for the user
        let insert_org = InsertOrganization::from_user(&insert_user)?;
        diesel::insert_into(schema::organization::table)
            .values(&insert_org)
            .execute(conn)
            .map_err(map_http_error!("Failed to signup user."))?;
        let organization_id = QueryOrganization::get_id(conn, &insert_org.uuid)?;

        // Connect the user to the organization as a `Leader`
        InsertOrganizationRole {
            user_id,
            organization_id,
            // TODO better type casting
            role: LEADER_ROLE.into(),
        }
    };

    diesel::insert_into(schema::organization_role::table)
        .values(&insert_org_role)
        .execute(conn)
        .map_err(map_http_error!("Failed to signup user."))?;

    let token = JsonWebToken::new_auth(&context.key, insert_user.email.clone())
        .map_err(map_http_error!("Failed to login user."))?;

    // TODO log this as trace if SMTP is configured
    info!("Confirm \"{}\" with: {token}", insert_user.email);

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(JsonEmpty::default()),
        CorsHeaders::new_pub("POST".into()),
    ))
}
