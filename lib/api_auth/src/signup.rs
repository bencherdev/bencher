use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseAccepted};
use bencher_json::{JsonSignup, system::auth::JsonAuthAck};
use bencher_schema::{
    context::{ApiContext, Body, ButtonBody, Message},
    error::{forbidden_error, issue_error},
    model::user::InsertUser,
    public_conn, write_conn,
};
use dropshot::{HttpError, RequestContext, TypedBody, endpoint};
use slog::Logger;

use super::AUTH_TOKEN_TTL;
use super::TOKEN_ARG;

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
/// When a user signs up, a new personal organization is automatically created.
/// Except when a user signs up with an invitation, then the user is just added to the inviting organization.
pub async fn auth_signup_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonSignup>,
) -> Result<ResponseAccepted<JsonAuthAck>, HttpError> {
    let json = post_inner(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        body.into_inner(),
    )
    .await?;
    Ok(Post::pub_response_accepted(json))
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    #[cfg(feature = "plus")] headers: &bencher_schema::HeaderMap,
    json_signup: JsonSignup,
) -> Result<JsonAuthAck, HttpError> {
    if !json_signup.i_agree {
        return Err(forbidden_error(
            "You must agree to the Bencher Terms of Use (https://bencher.dev/legal/terms-of-use), Privacy Policy (https://bencher.dev/legal/privacy), and License Agreement (https://bencher.dev/legal/license)",
        ));
    }
    #[cfg(feature = "plus")]
    crate::verify_recaptcha(
        log,
        context,
        headers,
        json_signup.recaptcha_token.as_ref(),
        bencher_json::RecaptchaAction::Signup,
    )
    .await?;

    let invited = json_signup.invite.is_some();
    let insert_user =
        InsertUser::from_json(write_conn!(context), &context.token_key, &json_signup)?;
    #[cfg(feature = "plus")]
    insert_user.rate_limit_auth(context)?;

    let token = context
        .token_key
        .new_auth(insert_user.email.clone(), AUTH_TOKEN_TTL)
        .map_err(|e| {
            issue_error(
                "Failed to create auth JWT at signup",
                &format!("Failed failed to create auth JWT ({insert_user:?} | {AUTH_TOKEN_TTL}) at signup"),
                e,
            )
        })?.to_string();

    let body = Body::Button(Box::new(ButtonBody {
        title: "Confirm Bencher Signup".into(),
        preheader: "Click the provided link to signup.".into(),
        greeting: format!("Ahoy {},", insert_user.name),
        pre_body: "Please, click the button below or use the provided token to signup for Bencher."
            .into(),
        button_text: "Confirm Email".into(),
        button_url: context
            .console_url
            .clone()
            .join("/auth/confirm")
            .map(|mut url| {
                #[cfg(feature = "plus")]
                if let Some(plan) = json_signup.plan {
                    url.query_pairs_mut()
                        .append_pair(super::PLAN_ARG, plan.as_ref());
                }
                url.query_pairs_mut().append_pair(TOKEN_ARG, &token);
                url.into()
            })
            .unwrap_or_default(),
        clipboard_text: "Confirmation Token".into(),
        clipboard_target: token,
        post_body: String::new(),
        closing: "See you soon,".into(),
        signature: "The Bencher Team".into(),
        settings_url: context
            .console_url
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

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserSignup(
        bencher_otel::AuthMethod::Email,
    ));

    insert_user.notify(
        log,
        public_conn!(context),
        &context.messenger,
        &context.console_url,
        invited,
        "email",
    )?;

    Ok(JsonAuthAck {
        email: insert_user.email,
    })
}
