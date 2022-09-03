use std::sync::Arc;

use bencher_json::{
    auth::JsonNonce,
    JsonLogin,
    JsonUser,
};
use chrono::Utc;
use diesel::{
    QueryDsl,
    RunQueryDsl,
};
use dropshot::{
    endpoint,
    HttpError,
    HttpResponseAccepted,
    HttpResponseHeaders,
    HttpResponseOk,
    Path,
    RequestContext,
    TypedBody,
};
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};

use crate::{
    db::{
        model::user::QueryUser,
        schema,
    },
    diesel::ExpressionMethods,
    util::{
        cors::get_cors,
        headers::CorsHeaders,
        http_error,
        Context,
    },
};
#[derive(Deserialize, JsonSchema)]
pub struct NonceParams {
    pub email: String,
    pub token: String,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/nonce",
    tags = ["auth"]
}]
pub async fn options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<NonceParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path = "/v0/auth/nonce",
    tags = ["auth"]
}]
pub async fn get(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<NonceParams>,
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonUser>, CorsHeaders>, HttpError> {
    let path_params = path_params.into_inner();
    let db_connection = rqctx.context();
    // let json_nonce = body.into_inner();

    // let conn = &mut *db_connection.lock().await;
    // let query_user = schema::user::table
    //     .filter(schema::user::email.eq(&json_login.email))
    //     .first::<QueryUser>(conn)
    //     .map_err(|_| http_error!("Failed to login user."))?;
    // let json_user = query_user.to_json()?;

    // Ok(HttpResponseHeaders::new(
    //     HttpResponseAccepted(json_user),
    //     CorsHeaders::new_pub("POST".into()),
    // ))

    todo!();
}
