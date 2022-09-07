use std::sync::Arc;

use bencher_json::{auth::JsonInvite, jwt::JsonWebToken, JsonEmpty, JsonLogin};
use diesel::{QueryDsl, RunQueryDsl};
use dropshot::{
    endpoint, HttpError, HttpResponseAccepted, HttpResponseHeaders, HttpResponseOk, RequestContext,
    TypedBody,
};
use tracing::info;

use crate::{
    db::{
        model::{
            organization::QueryOrganization,
            user::{organization::InsertOrganizationRole, InsertUser, QueryUser},
        },
        schema,
    },
    diesel::ExpressionMethods,
    util::{cors::get_cors, headers::CorsHeaders, http_error, Context},
};

#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/invite",
    tags = ["auth"]
}]
pub async fn options(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path = "/v0/auth/invite",
    tags = ["auth"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonInvite>,
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonEmpty>, CorsHeaders>, HttpError> {
    QueryUser::auth(&rqctx).await?;
    // TODO validate that user has the ability to invite users to said org

    let json_invite = body.into_inner();
    let context = &mut *rqctx.context().lock().await;

    let conn = &mut context.db;
    // Insert user
    let email = json_invite.signup.email.clone();
    let insert_user = InsertUser::from_json(conn, json_invite.signup)?;
    diesel::insert_into(schema::user::table)
        .values(&insert_user)
        .execute(conn)
        .map_err(|_| http_error!("Failed to invite user."))?;
    let user_id = QueryUser::get_id(conn, &insert_user.uuid)?;

    // Connect the user to the organization with the given role
    let organization_id = QueryOrganization::get_id(conn, &json_invite.organization)?;
    let insert_org_role = InsertOrganizationRole {
        user_id,
        organization_id,
        role: Role::from(json_invite.role).to_string(),
    };
    diesel::insert_into(schema::organization_role::table)
        .values(&insert_org_role)
        .execute(conn)
        .map_err(|_| http_error!("Failed to invite user."))?;

    let token = JsonWebToken::new_invite(&context.key, email)
        .map_err(|_| http_error!("Failed to invite user."))?;

    // TODO log this as trace if SMTP is configured
    info!("Accept invite for \"{}\" with: {token}", json_login.email);

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(JsonEmpty::default()),
        CorsHeaders::new_pub("POST".into()),
    ))
}
