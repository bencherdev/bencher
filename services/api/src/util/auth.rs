use std::str::FromStr;

use dropshot::{
    HttpError,
    RequestContext,
};
use uuid::Uuid;

use super::Context;

pub type AuthToken = Uuid;

pub async fn get_token(rqctx: &RequestContext<Context>) -> Result<AuthToken, HttpError> {
    let headers = rqctx.request.lock().await;
    let headers = headers
        .headers()
        .get("Authorization")
        .ok_or(HttpError::for_bad_request(
            Some("BadInput".into()),
            format!("Missing \"Authorization\" header."),
        ))?
        .to_str()
        .map_err(|e| {
            HttpError::for_bad_request(
                Some("BadInput".into()),
                format!("Invalid \"Authorization\" header."),
            )
        })?;
    let (_, uuid) = headers
        .split_once("Bearer ")
        .ok_or(HttpError::for_bad_request(
            Some("BadInput".into()),
            format!("Missisng \"Authorization\" Bearer."),
        ))?;
    Uuid::from_str(uuid).map_err(|_e| {
        HttpError::for_bad_request(
            Some("BadInput".into()),
            format!("Invalid \"Authorization\" Bearer token."),
        )
    })
}
