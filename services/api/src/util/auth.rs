use std::str::FromStr;

use bencher_json::token::JsonWebToken;
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
    RequestContext,
    TypedBody,
};
use uuid::Uuid;

use super::{
    http_error,
    Context,
};
use crate::db::model::user::QueryUser;

pub async fn get_token(rqctx: &RequestContext<Context>) -> Result<JsonWebToken, HttpError> {
    let request = rqctx.request.lock().await;
    let headers = request
        .headers()
        .get("Authorization")
        .ok_or(http_error!("Missing \"Authorization\" header."))?
        .to_str()
        .map_err(|_| http_error!("Invalid \"Authorization\" header."))?;
    let (_, token) = headers
        .split_once("Bearer ")
        .ok_or(http_error!("Missing \"Authorization\" Bearer."))?;
    Ok(token.to_string().into())
}
