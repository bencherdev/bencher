use bencher_json::system::auth::JsonAuth;
use bencher_json::JsonSignup;
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use http::StatusCode;
use slog::Logger;

use crate::endpoints::endpoint::CorsResponse;
use crate::endpoints::endpoint::Post;
use crate::endpoints::endpoint::{Endpoint, ResponseAccepted};
use crate::error::{forbidden_error, issue_error};

use crate::{
    context::{ApiContext, Body, ButtonBody, Message},
    model::user::InsertUser,
};

use super::AUTH_TOKEN_TTL;
use super::TOKEN_ARG;

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/signup",
    tags = ["auth"]
}]
pub async fn auth_signup_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

#[endpoint {
    method = POST,
    path =  "/v0/auth/signup",
    tags = ["auth"]
}]
pub async fn auth_signup_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonSignup>,
) -> Result<ResponseAccepted<JsonAuth>, HttpError> {
    let json = post_inner(&rqctx.log, rqctx.context(), body.into_inner()).await?;
    Ok(Post::pub_response_accepted(json))
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    json_signup: JsonSignup,
) -> Result<JsonAuth, HttpError> {
    if !json_signup.i_agree {
        return Err(forbidden_error(
            "You must agree to the Bencher Terms of Use (https://bencher.dev/legal/terms-of-use), Privacy Policy (https://bencher.dev/legal/privacy), and License Agreement (https://bencher.dev/legal/license)",
        ));
    }
    let conn = &mut *context.conn().await;

    #[cfg(feature = "plus")]
    let plan = json_signup.plan.unwrap_or_default();

    let invited = json_signup.invite.is_some();
    let insert_user = InsertUser::insert_from_json(conn, &context.token_key, &json_signup)?;

    let token = context
        .token_key
        .new_auth(insert_user.email.clone(), AUTH_TOKEN_TTL)
        .map_err(|e| {
            issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create auth JWT at signup",
                &format!("Failed failed to create auth JWT ({insert_user:?} | {AUTH_TOKEN_TTL}) at signup"),
                e,
            )
        })?.to_string();

    let body = Body::Button(Box::new(ButtonBody {
        title: "Confirm Bencher Signup".into(),
        preheader: "Click the provided link to signup.".into(),
        greeting: format!("Ahoy {},", insert_user.name),
        pre_body: "Please, click the button below or use the provided code to signup for Bencher."
            .into(),
        button_text: "Confirm Email".into(),
        button_url: context
            .endpoint
            .clone()
            .join("/auth/confirm")
            .map(|mut url| {
                #[cfg(feature = "plus")]
                url.query_pairs_mut()
                    .append_pair(super::PLAN_ARG, plan.as_ref());
                url.query_pairs_mut().append_pair(TOKEN_ARG, &token);
                url.into()
            })
            .unwrap_or_default(),
        clipboard_text: "Confirmation Code".into(),
        clipboard_target: token,
        post_body: String::new(),
        closing: "See you soon,".into(),
        signature: "The Bencher Team".into(),
        settings_url: context
            .endpoint
            .clone()
            .join("/help")
            .map(Into::into)
            .unwrap_or_default(),
    }));
    let message = Message {
        to_name: Some(insert_user.name.clone().into()),
        to_email: insert_user.email.clone().into(),
        subject: Some("Confirm Bencher Signup".into()),
        body: Some(body),
    };
    context.messenger.send(log, message);

    insert_user.notify(
        log,
        conn,
        &context.messenger,
        &context.endpoint,
        invited,
        "email",
    )?;

    Ok(JsonAuth {
        email: insert_user.email,
    })
}
