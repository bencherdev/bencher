use std::sync::Arc;

use bencher_json::{auth::JsonInvite, jwt::JsonWebToken, JsonEmpty};

use dropshot::{
    endpoint, HttpError, HttpResponseAccepted, HttpResponseHeaders, HttpResponseOk, RequestContext,
    TypedBody,
};
use tracing::info;

use crate::{
    model::user::QueryUser,
    util::{cors::get_cors, headers::CorsHeaders, map_http_error, Context},
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
    // TODO validate that user has the ability to invite users to said org
    QueryUser::auth(&rqctx).await?;

    let json_invite = body.into_inner();
    let context = &mut *rqctx.context().lock().await;
    let token = JsonWebToken::new_invite(
        &context.key,
        json_invite.email.clone(),
        json_invite.organization,
        json_invite.role,
    )
    .map_err(map_http_error!("Failed to invite user."))?;

    // TODO log this as trace if SMTP is configured
    info!("Accept invite for \"{}\" with: {token}", json_invite.email);

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(JsonEmpty::default()),
        CorsHeaders::new_pub("POST".into()),
    ))
}
