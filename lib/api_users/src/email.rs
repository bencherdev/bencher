//! The emails of an email change: the confirm link to the new address, and the notices to the old
//! address and, when a server admin changes it, to the new one.

use bencher_json::{Email, EmailChangeSecret};
use bencher_schema::{
    auth_conn,
    context::{ApiContext, Body, Button, ButtonBody, Clipboard, Message},
    error::{
        BencherResource, issue_error, resource_conflict_error, resource_not_found_err,
        service_unavailable_error,
    },
    model::user::{QueryUser, email_change::EMAIL_CHANGE_LIFETIME},
    schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;
use slog::Logger;

/// The console's page an email change's link opens, with its secret in the fragment.
const CONFIRM_PATH: &str = "/auth/email";
const CONFIRM_SUBJECT: &str = "Confirm Bencher Email Change";
const ASKED_SUBJECT: &str = "Bencher Email Change Asked For";
const CHANGED_SUBJECT: &str = "Bencher Email Changed";
const TAKEN: &str = "The email is already in use";

/// Refuse `new_email` for the user while another user has it, charged to the user's sign-in
/// attempts.
pub async fn check_new_email(
    context: &ApiContext,
    query_user: &QueryUser,
    new_email: &Email,
) -> Result<(), HttpError> {
    #[cfg(feature = "plus")]
    query_user.rate_limit_auth(context)?;
    let in_use = diesel::select(diesel::dsl::exists(
        schema::user::table.filter(schema::user::email.eq(new_email)),
    ))
    .get_result::<bool>(auth_conn!(context))
    .map_err(resource_not_found_err!(User, new_email))?;
    if in_use {
        return Err(taken(query_user, new_email));
    }
    Ok(())
}

/// The refusal of a new email another user has.
pub fn taken(query_user: &QueryUser, new_email: &Email) -> HttpError {
    resource_conflict_error(BencherResource::User, (query_user.uuid, new_email), TAKEN)
}

/// Email the old address that a change to `new_email` was asked for, then the new address its
/// link, which carries `secret` in its fragment, and wait until each is sent.
///
/// The notice goes first, so no link is ever out that its old address was not told of.
pub async fn send_asked(
    log: &Logger,
    context: &ApiContext,
    query_user: &QueryUser,
    new_email: &Email,
    secret: &EmailChangeSecret,
) -> Result<(), HttpError> {
    let minutes = EMAIL_CHANGE_LIFETIME.num_minutes();
    let notice = notice(
        context,
        query_user,
        &query_user.email,
        ASKED_SUBJECT,
        format!(
            "Someone asked to change the email of your Bencher account from {old} to {new_email}. \
             It changes only once {new_email} confirms it, in the next {minutes} minutes.",
            old = query_user.email,
        ),
        "If you did not ask for this, sign in to Bencher and change your email.".into(),
    );
    send(log, context, notice).await?;

    let mut button_url = context.console_url.join(CONFIRM_PATH).map_err(|e| {
        issue_error(
            "Failed to make an email change link",
            "Failed to make an email change link:",
            e,
        )
    })?;
    button_url.set_fragment(Some(&format!("secret={}", secret.as_ref())));
    let body = Body::Button(Box::new(ButtonBody {
        title: CONFIRM_SUBJECT.into(),
        preheader: "Click the provided link to confirm your new email.".into(),
        greeting: format!("Ahoy {},", query_user.name),
        pre_body: "Someone asked to change the email of a Bencher account to this address. Please, click the button below to confirm the change.".into(),
        button: Some(Button {
            text: "Confirm Email".into(),
            url: button_url.into(),
        }),
        clipboard: Some(Clipboard {
            text: "New Email".into(),
            target: new_email.to_string(),
        }),
        post_body: format!(
            "The button works once, in the next {minutes} minutes. If you did not ask for this, you can ignore this email."
        ),
        closing: "See you soon,".into(),
        signature: "The Bencher Team".into(),
        settings_url: settings_url(context),
    }));
    let link = Message {
        to_name: Some(query_user.name.clone().into()),
        to_email: new_email.to_string(),
        subject: Some(CONFIRM_SUBJECT.into()),
        body: Some(body),
    };
    send(log, context, link).await
}

/// Email both addresses that a server admin changed the user's email from `old_email` to the
/// user's email, and wait until each is sent.
pub async fn send_changed(
    log: &Logger,
    context: &ApiContext,
    query_user: &QueryUser,
    old_email: &Email,
) -> Result<(), HttpError> {
    let new_email = &query_user.email;
    let to_old = notice(
        context,
        query_user,
        old_email,
        CHANGED_SUBJECT,
        format!(
            "A server admin changed the email of your Bencher account from {old_email} to {new_email}. Every session of the account has ended, and its legacy API tokens are revoked."
        ),
        "If you did not expect this, contact the admin of your Bencher server.".into(),
    );
    send(log, context, to_old).await?;
    let to_new = notice(
        context,
        query_user,
        new_email,
        CHANGED_SUBJECT,
        format!(
            "A server admin changed the email of a Bencher account from {old_email} to this address."
        ),
        "Sign in to Bencher with this address from now on.".into(),
    );
    send(log, context, to_new).await
}

/// A notice to `to_email` about the user's email, with no button.
fn notice(
    context: &ApiContext,
    query_user: &QueryUser,
    to_email: &Email,
    subject: &str,
    pre_body: String,
    post_body: String,
) -> Message {
    let body = Body::Button(Box::new(ButtonBody {
        title: subject.into(),
        preheader: pre_body.clone(),
        greeting: format!("Ahoy {},", query_user.name),
        pre_body,
        button: None,
        clipboard: None,
        post_body,
        closing: "See you soon,".into(),
        signature: "The Bencher Team".into(),
        settings_url: settings_url(context),
    }));
    Message {
        to_name: Some(query_user.name.clone().into()),
        to_email: to_email.to_string(),
        subject: Some(subject.into()),
        body: Some(body),
    }
}

async fn send(log: &Logger, context: &ApiContext, message: Message) -> Result<(), HttpError> {
    context
        .messenger
        .send_and_wait(log, message)
        .await
        .map_err(|e| {
            slog::error!(log, "Failed to send an email change email: {e}");
            service_unavailable_error(format!("Failed to send the email change email: {e}"))
        })
}

fn settings_url(context: &ApiContext) -> String {
    context
        .console_url
        .clone()
        .join("/help")
        .map(Into::into)
        .unwrap_or_default()
}
