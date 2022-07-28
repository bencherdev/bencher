use std::str::FromStr;

use dropshot::{
    HttpError,
    RequestContext,
};
use uuid::Uuid;

use super::{
    http_error,
    Context,
};

pub type AuthToken = Uuid;

pub async fn get_token(rqctx: &RequestContext<Context>) -> Result<AuthToken, HttpError> {
    let headers = rqctx.request.lock().await;
    let headers = headers
        .headers()
        .get("Authorization")
        .ok_or(http_error!("Missing \"Authorization\" header."))?
        .to_str()
        .map_err(|_| http_error!("Invalid \"Authorization\" header."))?;
    let (_, uuid) = headers
        .split_once("Bearer ")
        .ok_or(http_error!("Missisng \"Authorization\" Bearer."))?;
    Uuid::from_str(uuid).map_err(|_| http_error!("Invalid \"Authorization\" Bearer token."))
}
