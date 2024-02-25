use bencher_json::system::auth::JsonAccept;
use bencher_json::JsonAuthAck;
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};

use crate::conn;
use crate::endpoints::endpoint::CorsResponse;
use crate::endpoints::endpoint::Get;
use crate::endpoints::endpoint::Post;
use crate::endpoints::endpoint::ResponseAccepted;
use crate::endpoints::Endpoint;

use crate::context::ApiContext;
use crate::model::user::auth::AuthUser;
use crate::model::user::auth::BearerToken;

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/accept",
    tags = ["auth", "organizations"]
}]
pub async fn auth_accept_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

#[endpoint {
    method = POST,
    path = "/v0/auth/accept",
    tags = ["auth", "organizations"]
}]
pub async fn auth_accept_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    body: TypedBody<JsonAccept>,
) -> Result<ResponseAccepted<JsonAuthAck>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(rqctx.context(), body.into_inner(), auth_user).await?;
    Ok(Post::auth_response_accepted(json))
}

async fn post_inner(
    context: &ApiContext,
    json_accept: JsonAccept,
    auth_user: AuthUser,
) -> Result<JsonAuthAck, HttpError> {
    auth_user.user.check_is_locked()?;
    auth_user
        .user
        .accept_invite(conn!(context), &context.token_key, &json_accept.invite)?;

    Ok(JsonAuthAck {
        email: auth_user.user.email,
    })
}
