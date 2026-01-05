use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseAccepted};
use bencher_json::{JsonAuthAck, JsonLogin};
use bencher_schema::{
    conn_lock,
    context::{ApiContext, Body, ButtonBody, Message},
    error::issue_error,
    model::user::QueryUser,
};
use dropshot::{HttpError, RequestContext, TypedBody, endpoint};
use slog::Logger;

use super::AUTH_TOKEN_TTL;
use super::TOKEN_ARG;

#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/login",
    tags = ["auth"]
}]
pub async fn auth_login_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

#[endpoint {
    method = POST,
    path = "/v0/auth/login",
    tags = ["auth"]
}]
pub async fn auth_login_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonLogin>,
) -> Result<ResponseAccepted<JsonAuthAck>, HttpError> {
    let json = post_inner(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        &rqctx.request_id,
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
    #[cfg(feature = "plus")] request_id: &str,
    #[cfg(feature = "plus")] headers: &bencher_schema::HeaderMap,
    json_login: JsonLogin,
) -> Result<JsonAuthAck, HttpError> {
    #[cfg(feature = "plus")]
    crate::verify_recaptcha(
        log,
        request_id,
        context,
        headers,
        json_login.recaptcha_token.as_ref(),
        bencher_json::RecaptchaAction::Login,
    )
    .await?;

    let query_user = QueryUser::get_with_email(conn_lock!(context), &json_login.email)?;
    query_user.check_is_locked()?;
    #[cfg(feature = "plus")]
    query_user.rate_limit_auth(context)?;

    if let Some(invite) = &json_login.invite {
        query_user.accept_invite(conn_lock!(context), &context.token_key, invite)?;

        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserAccept(Some(
            bencher_otel::AuthMethod::Email,
        )));
    }

    let token = context
        .token_key
        .new_auth(json_login.email.clone(), AUTH_TOKEN_TTL)
        .map_err(|e| {
            issue_error(
                "Failed to create auth JWT at login",
                &format!(
                    "Failed failed to create auth JWT ({json_login:?} | {AUTH_TOKEN_TTL}) at login"
                ),
                e,
            )
        })?
        .to_string();

    let body = Body::Button(Box::new(ButtonBody {
        title: "Confirm Bencher Login".into(),
        preheader: "Click the provided link to login.".into(),
        greeting: format!("Ahoy {},", query_user.name),
        pre_body: "Please, click the button below or use the provided token to login to Bencher."
            .into(),
        button_text: "Confirm Login".into(),
        button_url: context
            .console_url
            .clone()
            .join("/auth/confirm")
            .map(|mut url| {
                #[cfg(feature = "plus")]
                if let Some(plan) = json_login.plan {
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
        to_name: Some(query_user.name.into()),
        to_email: query_user.email.into(),
        subject: Some("Confirm Bencher Login".into()),
        body: Some(body),
    };
    context.messenger.send(log, message);

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserLogin(
        bencher_otel::AuthMethod::Email,
    ));

    Ok(JsonAuthAck {
        email: json_login.email,
    })
}
