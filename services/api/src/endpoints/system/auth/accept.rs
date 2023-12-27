use bencher_json::system::auth::JsonAcceptInvite;
use bencher_json::JsonAuth;
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};

use crate::endpoints::endpoint::CorsResponse;
use crate::endpoints::endpoint::Get;
use crate::endpoints::endpoint::Post;
use crate::endpoints::endpoint::ResponseAccepted;
use crate::endpoints::Endpoint;

use crate::model::user::auth::AuthUser;
use crate::model::user::auth::BearerToken;
use crate::{context::ApiContext, model::user::QueryUser};

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
    body: TypedBody<JsonAcceptInvite>,
) -> Result<ResponseAccepted<JsonAuth>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(rqctx.context(), body.into_inner(), &auth_user).await?;
    Ok(Post::auth_response_accepted(json))
}

async fn post_inner(
    context: &ApiContext,
    json_accept_invite: JsonAcceptInvite,
    auth_user: &AuthUser,
) -> Result<JsonAuth, HttpError> {
    let conn = &mut *context.conn().await;

    let query_user = QueryUser::get(conn, auth_user.id)?;
    query_user.check_is_locked()?;
    query_user.accept_invite(conn, &context.token_key, &json_accept_invite.invite)?;

    Ok(JsonAuth {
        email: query_user.email,
    })
}
