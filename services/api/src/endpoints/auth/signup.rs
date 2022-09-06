use std::sync::Arc;

use bencher_json::{jwt::JsonWebToken, JsonEmpty, JsonSignup};
use diesel::dsl::count;
use diesel::QueryDsl;
use diesel::ExpressionMethods;
use diesel::RunQueryDsl;
use dropshot::{
    endpoint, HttpError, HttpResponseAccepted, HttpResponseHeaders, HttpResponseOk, RequestContext,
    TypedBody,
};
use tracing::info;

use crate::db::model::organization::InsertOrganization;
use crate::{
    db::{model::user::InsertUser, schema},
    util::{cors::get_cors, headers::CorsHeaders, http_error, Context},
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
    let json_signup = body.into_inner();
    let context = &mut *rqctx.context().lock().await;

    let conn = &mut context.db;
    let mut insert_user = InsertUser::from_json(conn, json_signup)?;

    let count = schema::user::table
        .select(count(schema::user::id))
        .first::<i64>(conn)
        .map_err(|_| http_error!("Failed to signup user."))?;
    // The first user to signup is admin
    if count == 0 {
        insert_user.admin = true;
    }

    let insert_org = InsertOrganization::from_user(conn, &insert_user)?;
    diesel::insert_into(schema::organization::table)
        .values(&insert_org)
        .execute(conn)
        .map_err(|_| http_error!("Failed to signup user."))?;
    let org_id = schema::organization::table
        .filter(schema::organization::uuid.eq(&insert_org.uuid))
        .select(schema::organization::id)
        .first::<i32>(conn)
        .map_err(|_| http_error!("Failed to create organization."))?;

    diesel::insert_into(schema::user::table)
        .values(&insert_user)
        .execute(conn)
        .map_err(|_| http_error!("Failed to signup user."))?;

    let token = JsonWebToken::new_auth(&context.key, insert_user.email.clone())
        .map_err(|_| http_error!("Failed to login user."))?;

    // TODO log this as trace if SMTP is configured
    info!("Confirm \"{}\" with: {token}", insert_user.email);

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(JsonEmpty::default()),
        CorsHeaders::new_pub("POST".into()),
    ))
}
