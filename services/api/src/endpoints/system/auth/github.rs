#![cfg(feature = "plus")]

use bencher_json::JsonAuthUser;

use bencher_json::system::auth::JsonOAuth;
use bencher_json::JsonSignup;
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use http::StatusCode;
use slog::Logger;

use crate::endpoints::endpoint::CorsResponse;
use crate::endpoints::endpoint::Get;
use crate::endpoints::endpoint::Post;
use crate::endpoints::endpoint::ResponseAccepted;
use crate::endpoints::Endpoint;

use crate::error::issue_error;
use crate::error::payment_required_error;
use crate::error::unauthorized_error;
use crate::model::organization::plan::LicenseUsage;
use crate::model::user::InsertUser;
use crate::{context::ApiContext, model::user::QueryUser};

use super::CLIENT_TOKEN_TTL;

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/github",
    tags = ["auth"]
}]
pub async fn auth_github_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

#[endpoint {
    method = POST,
    path = "/v0/auth/github",
    tags = ["auth"]
}]
pub async fn auth_github_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonOAuth>,
) -> Result<ResponseAccepted<JsonAuthUser>, HttpError> {
    let json = post_inner(&rqctx.log, rqctx.context(), body.into_inner()).await?;
    Ok(Post::pub_response_accepted(json))
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    json_oauth: JsonOAuth,
) -> Result<JsonAuthUser, HttpError> {
    let Some(github) = &context.github else {
        let err = "GitHub OAuth2 is not configured";
        slog::warn!(log, "{err}");
        return Err(payment_required_error(err));
    };
    let conn = &mut *context.conn().await;
    // If not on Bencher Cloud, then at least one organization must have a valid Bencher Plus license
    if !context.is_bencher_cloud()
        && LicenseUsage::get_for_server(conn, &context.licensor)?.is_empty()
    {
        return Err(payment_required_error(
                "You must have a valid Bencher Plus license for at least one organization on the server to use GitHub OAuth2",
            ));
    }

    let (name, email) = github
        .oauth_user(json_oauth.code)
        .await
        .map_err(unauthorized_error)?;

    // If the user already exists, then we just need to check if they are locked and possible accept an invite
    // Otherwise, we need to create a new user and notify the admins
    let user = if let Ok(query_user) = QueryUser::get_with_email(conn, &email) {
        query_user.check_is_locked()?;
        if let Some(invite) = &json_oauth.invite {
            query_user.accept_invite(conn, &context.token_key, invite)?;
        }
        query_user
    } else {
        let json_signup = JsonSignup {
            name,
            slug: None,
            email: email.clone(),
            i_agree: true,
            invite: json_oauth.invite.clone(),
            plan: None,
        };

        let invited = json_signup.invite.is_some();
        let insert_user = InsertUser::insert_from_json(conn, &context.token_key, &json_signup)?;

        insert_user.notify(
            log,
            conn,
            &context.messenger,
            &context.endpoint,
            invited,
            "GitHub OAuth2",
        )?;

        QueryUser::get_with_email(conn, &email)?
    }
    .into_json();

    let token = context
        .token_key
        .new_client(email.clone(), CLIENT_TOKEN_TTL)
        .map_err(|e| {
            issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create client JWT for GitHub OAuth2",
                &format!(
                    "Failed to create client JWT for GitHub OAuth2 ({email} | {CLIENT_TOKEN_TTL})"
                ),
                e,
            )
        })?;

    Ok(JsonAuthUser { user, token })
}
