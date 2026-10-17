use bencher_endpoint::{CorsResponse, Endpoint, Put, ResponseOk};
use bencher_json::{DateTime, Email, JsonConfirm, system::auth::JsonAuthUser};
#[cfg(feature = "plus")]
use bencher_schema::model::organization::sso::QuerySso;
use bencher_schema::{
    AUTH_TOKEN_TTL, CLIENT_TOKEN_TTL, auth_conn,
    context::{ApiContext, Body, ButtonBody, Message},
    error::{
        BencherResource, forbidden_error, issue_error, resource_conflict_err,
        resource_conflict_error, resource_not_found_err, unauthorized_error,
    },
    model::user::{QueryUser, bump_modified, token::QueryToken},
    public_conn, schema, write_transaction,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{HttpError, Path, RequestContext, TypedBody, endpoint};

use crate::users::UserParams;

const SUBJECT: &str = "Confirm Bencher Email Change";
const USER_ARG: &str = "user";
const TOKEN_ARG: &str = "token";

#[endpoint {
    method = OPTIONS,
    path =  "/v0/users/{user}/email",
    tags = ["users"]
}]
pub async fn user_email_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<UserParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Put.into()]))
}

/// Confirm a new email
///
/// Change a user's email to the new email that a confirm link was sent to.
/// A user who updates their own email is sent that link at the new email,
/// and its token lasts as long as a login link's.
/// No credential is needed, so the link works from any device.
/// Any later update to the user ends the link, this confirm included.
/// The response holds a new session for the new email.
/// An invalid or expired token is refused with `401 Unauthorized`;
/// a token for another user, or one sent before the user's latest update, with `403 Forbidden`;
/// and a new email that another user has taken since with `409 Conflict`.
#[endpoint {
    method = PUT,
    path =  "/v0/users/{user}/email",
    tags = ["users"]
}]
pub async fn user_email_put(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<UserParams>,
    body: TypedBody<JsonConfirm>,
) -> Result<ResponseOk<JsonAuthUser>, HttpError> {
    let json = put_inner(rqctx.context(), path_params.into_inner(), body.into_inner()).await?;
    Ok(Put::pub_response_ok(json))
}

async fn put_inner(
    context: &ApiContext,
    path_params: UserParams,
    json_confirm: JsonConfirm,
) -> Result<JsonAuthUser, HttpError> {
    let now = context.clock.now();
    let claims = context
        .token_key
        .validate_email_change(&json_confirm.token, now)
        .map_err(unauthorized_error)?;
    let query_user = QueryUser::from_resource_id(public_conn!(context), &path_params.user)?;
    if query_user.uuid != claims.sub {
        return Err(forbidden_error(
            "This link confirms a new email for another user.",
        ));
    }
    query_user.check_is_locked()?;

    let new_email = claims.new_email;
    // Every write to a user moves `modified`, so a link ends with the user's next update.
    let changed = write_transaction!(context, |conn| {
        let changed = diesel::update(
            schema::user::table
                .filter(schema::user::id.eq(query_user.id))
                .filter(schema::user::email.eq(&claims.email))
                .filter(schema::user::modified.eq(claims.modified)),
        )
        .set((schema::user::email.eq(&new_email), bump_modified(now)))
        .execute(conn)?;
        if changed > 0 {
            QueryToken::revoke_all(conn, query_user.id, now)?;
        }
        diesel::QueryResult::Ok(changed)
    })
    .map_err(resource_conflict_err!(User, (query_user.uuid, &new_email)))?;
    if changed == 0 {
        return Err(forbidden_error(
            "This link is no longer valid: the user has changed since it was sent.",
        ));
    }
    let query_user = QueryUser::get(public_conn!(context), query_user.id)?;

    #[cfg(feature = "plus")]
    QuerySso::join_all(
        context,
        &query_user,
        #[cfg(feature = "otel")]
        bencher_otel::AuthMethod::Email,
    )
    .await?;

    let token = context
        .token_key
        .new_client(new_email.clone(), CLIENT_TOKEN_TTL)
        .map_err(|e| {
            issue_error(
                "Failed to create client JWT",
                &format!(
                    "Failed to create client JWT for a new email ({new_email} | {CLIENT_TOKEN_TTL})"
                ),
                e,
            )
        })?;
    let client_claims = context.token_key.validate_client(&token).map_err(|e| {
        issue_error(
            "Failed to validate new client JWT",
            &format!("Failed to validate new client JWT for a new email ({new_email})"),
            e,
        )
    })?;

    Ok(JsonAuthUser {
        user: query_user.into_json(),
        token,
        creation: client_claims.issued_at(),
        expiration: client_claims.expiration(),
    })
}

/// Refuse `new_email` for the user while another user has it.
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
        return Err(resource_conflict_error(
            BencherResource::User,
            (query_user.uuid, new_email),
            "The email is already in use",
        ));
    }
    Ok(())
}

/// The email that sends `new_email` a link to confirm it as the user's email,
/// valid while the user's `modified` is still `modified`.
pub fn confirm_message(
    context: &ApiContext,
    query_user: &QueryUser,
    modified: DateTime,
    new_email: Email,
) -> Result<Message, HttpError> {
    let token = context
        .token_key
        .new_email_change(
            context.clock.now(),
            AUTH_TOKEN_TTL,
            query_user.uuid,
            modified,
            query_user.email.clone(),
            new_email.clone(),
        )
        .map_err(|e| {
            issue_error(
                "Failed to create email change JWT",
                &format!(
                    "Failed to create email change JWT for user ({uuid} | {AUTH_TOKEN_TTL})",
                    uuid = query_user.uuid
                ),
                e,
            )
        })?;

    let body = Body::Button(Box::new(ButtonBody {
        title: SUBJECT.into(),
        preheader: "Click the provided link to confirm your new email.".into(),
        greeting: format!("Ahoy {},", query_user.name),
        pre_body: "Someone asked to change the email of a Bencher account to this address. Please, click the button below to confirm the change.".into(),
        button_text: "Confirm Email".into(),
        button_url: context
            .console_url
            .clone()
            .join("/auth/email")
            .map(|mut url| {
                url.query_pairs_mut()
                    .append_pair(USER_ARG, &query_user.uuid.to_string())
                    .append_pair(TOKEN_ARG, token.as_ref());
                url.into()
            })
            .unwrap_or_default(),
        clipboard_text: "New Email".into(),
        clipboard_target: new_email.to_string(),
        post_body: "If you did not ask for this, you can ignore this email.".into(),
        closing: "See you soon,".into(),
        signature: "The Bencher Team".into(),
        settings_url: context
            .console_url
            .clone()
            .join("/help")
            .map(Into::into)
            .unwrap_or_default(),
    }));
    Ok(Message {
        to_name: Some(query_user.name.clone().into()),
        to_email: new_email.into(),
        subject: Some(SUBJECT.into()),
        body: Some(body),
    })
}
