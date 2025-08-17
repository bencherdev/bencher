#![cfg(feature = "plus")]

use bencher_endpoint::{CorsResponse, Endpoint, Get, Post, ResponseAccepted};
use bencher_json::{JsonAuthUser, JsonSignup, PlanLevel, system::auth::JsonOAuth};
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    error::{issue_error, payment_required_error, unauthorized_error},
    model::{
        organization::{QueryOrganization, plan::LicenseUsage},
        user::{InsertUser, QueryUser},
    },
};
use dropshot::{HttpError, RequestContext, TypedBody, endpoint};
use slog::Logger;

use super::CLIENT_TOKEN_TTL;

pub const GOOGLE_OAUTH2: &str = "Google OAuth2";

#[endpoint {
    method = OPTIONS,
    path =  "/v0/auth/google",
    tags = ["auth"]
}]
pub async fn auth_google_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

#[endpoint {
    method = POST,
    path = "/v0/auth/google",
    tags = ["auth"]
}]
pub async fn auth_google_post(
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
    let Some(google_client) = &context.google_client else {
        let err = "Google OAuth2 is not configured";
        slog::warn!(log, "{err}");
        return Err(payment_required_error(err));
    };
    // If not on Bencher Cloud, then at least one organization must have a valid Bencher Plus license
    if !context.is_bencher_cloud
        && LicenseUsage::get_for_server(
            &context.database.connection,
            &context.licensor,
            Some(PlanLevel::Enterprise),
        )
        .await?
        .is_empty()
    {
        return Err(payment_required_error(
            "You must have a valid Bencher Plus Enterprise license for at least one organization on the server to use Google OAuth2",
        ));
    }

    let (name, email) = google_client
        .oauth_user(json_oauth.code)
        .await
        .map_err(unauthorized_error)?;

    // If the user already exists, then we just need to check if they are locked and possible accept an invite
    // Otherwise, we need to create a new user and notify the admins
    let query_user = QueryUser::get_with_email(conn_lock!(context), &email);
    let user = if let Ok(query_user) = query_user {
        query_user.check_is_locked()?;
        if let Some(invite) = &json_oauth.invite {
            query_user.accept_invite(conn_lock!(context), &context.token_key, invite)?;
        } else if let Some(organization_uuid) = json_oauth.claim {
            let query_organization =
                QueryOrganization::from_uuid(conn_lock!(context), organization_uuid)?;
            query_organization.claim(context, &query_user).await?;
        }
        query_user
    } else {
        let json_signup = JsonSignup {
            name,
            slug: None,
            email: email.clone(),
            plan: json_oauth.plan,
            invite: json_oauth.invite.clone(),
            claim: json_oauth.claim,
            i_agree: true,
        };

        let invited = json_signup.invite.is_some();
        let insert_user =
            InsertUser::from_json(conn_lock!(context), &context.token_key, &json_signup)?;

        insert_user.notify(
            log,
            conn_lock!(context),
            &context.messenger,
            &context.console_url,
            invited,
            GOOGLE_OAUTH2,
        )?;

        QueryUser::get_with_email(conn_lock!(context), &email)?
    }
    .into_json();

    let token = context
        .token_key
        .new_client(email.clone(), CLIENT_TOKEN_TTL)
        .map_err(|e| {
            issue_error(
                "Failed to create client JWT for GitHub OAuth2",
                &format!(
                    "Failed to create client JWT for GitHub OAuth2 ({email} | {CLIENT_TOKEN_TTL})"
                ),
                e,
            )
        })?;

    let claims = context.token_key.validate_client(&token).map_err(|e| {
        issue_error(
            "Failed to validate new client JWT for GitHub OAuth2",
            &format!("Failed to validate new client JWT for GitHub OAuth2: {token}"),
            e,
        )
    })?;

    Ok(JsonAuthUser {
        user,
        token,
        creation: claims.issued_at(),
        expiration: claims.expiration(),
    })
}
