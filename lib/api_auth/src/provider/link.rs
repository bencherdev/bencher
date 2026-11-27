//! The one rule that links a provider account to a user. A pinned account signs its user in. A new
//! address signs up, pinned at birth. An address that has an account proves it by email first, and
//! a proof in the browser that ran the provider's sign-in pins the provider account.

use bencher_endpoint::{Post, ResponseOk};
use bencher_json::{DateTime, Email, JsonProviderInbox, JsonProviderSignIn, Jwt};
use bencher_schema::{
    context::{ApiContext, DbConnection},
    model::{
        organization::sso::QuerySso,
        user::{InsertUser, QueryUser, sign_in::SignInContext},
    },
    write_transaction,
};
use dropshot::{HttpError, RequestInfo};
use http::header::SET_COOKIE;
use slog::Logger;

use super::ProviderAccount;
use crate::sign_in::{
    Emailed, Presented, SignedIn, TxError, begin_session, email_sign_in, join, locked_refusal,
    resend_after,
};

/// Link the provider account by the one rule, and answer: signed in, or the address to prove.
pub(super) async fn link(
    log: &Logger,
    context: &ApiContext,
    request: &RequestInfo,
    account: ProviderAccount,
    carried: SignInContext,
) -> Result<ResponseOk<JsonProviderSignIn>, HttpError> {
    let rule = Rule {
        context,
        presented: Presented::new(context, request),
        account: &account,
        carried: &carried,
        now: context.clock.now(),
    };
    let linked =
        write_transaction!(context, |conn| rule.apply(conn)).map_err(TxError::into_http)?;
    match linked {
        Linked::SignedIn(signed_in) => {
            let (json, session_cookie) = signed_in.finish(log, context).await?;
            let mut response = Post::pub_response_ok(JsonProviderSignIn {
                signed_in: Some(json),
                inbox: None,
            });
            response.headers_mut().append(SET_COOKIE, session_cookie);
            Ok(response)
        },
        Linked::Locked => Err(locked_refusal(context)),
        Linked::Unproven => prove_first(log, context, request, account, carried).await,
    }
}

/// What the rule did, inside the sign-in's write transaction.
enum Linked {
    SignedIn(Box<SignedIn>),
    Locked,
    /// The address has an account the provider account is not pinned to.
    Unproven,
}

/// The rule, for one provider account that signed in.
struct Rule<'a> {
    context: &'a ApiContext,
    presented: Presented<'a>,
    account: &'a ProviderAccount,
    carried: &'a SignInContext,
    now: DateTime,
}

impl Rule<'_> {
    fn apply(&self, conn: &mut DbConnection) -> Result<Linked, TxError> {
        let Self { account, now, .. } = self;
        if let Some(user) = account.pin.user(conn)? {
            if user.locked {
                return Ok(Linked::Locked);
            }
            account.pin.refresh(conn)?;
            return self.sign_in(conn, user, None);
        }
        if QueryUser::find_with_email(conn, &account.email)?.is_some() {
            return Ok(Linked::Unproven);
        }
        let (insert_user, user) =
            InsertUser::sign_up(conn, account.name.clone(), None, account.email.clone())?;
        // Pinned to no one, as this same transaction found above.
        let _pinned = account.pin.pin(conn, user.id, *now)?;
        self.sign_in(conn, user, Some(insert_user))
    }

    /// Sign the user in by the provider: join what the sign-in carries, record the address the
    /// provider verified, join by Google's hosted domain, and begin the session.
    fn sign_in(
        &self,
        conn: &mut DbConnection,
        user: QueryUser,
        new: Option<InsertUser>,
    ) -> Result<Linked, TxError> {
        let Self {
            context,
            presented,
            account,
            carried,
            now,
        } = self;
        let invite = invite_for(context, carried.invite.as_ref(), &account.email)
            .filter(|_| user.email == account.email);
        let (invited, claimed) = join(conn, context, invite, carried.claim, &user, *now)?;
        if new.is_some() && !invited && !claimed {
            InsertUser::personal_organization(conn, &user)?;
        }
        QueryUser::verify_email(conn, user.id, &account.email, *now)?;
        let sso_joined = if let Some(domain) = &account.hosted_domain {
            QuerySso::join_domain_with(conn, &user, domain.as_ref(), *now)?
        } else {
            false
        };
        let provider = Some(account.pin.provider);
        let token = begin_session(conn, &user, presented, provider, *now)?;
        Ok(Linked::SignedIn(Box::new(SignedIn {
            user,
            new,
            invited,
            claimed,
            sso_joined,
            provider,
            token,
            context: (*carried).clone(),
        })))
    }
}

/// The invite, if it is valid and for `email`: a provider proves only the address it verified.
fn invite_for<'a>(context: &ApiContext, invite: Option<&'a Jwt>, email: &Email) -> Option<&'a Jwt> {
    invite.filter(|invite| {
        context
            .token_key
            .validate_invite(invite)
            .is_ok_and(|claims| claims.email() == email)
    })
}

/// Email the address, to prove it before the provider account pins, carrying the pin and what the
/// sign-in carries, under the address's cap.
async fn prove_first(
    log: &Logger,
    context: &ApiContext,
    request: &RequestInfo,
    account: ProviderAccount,
    carried: SignInContext,
) -> Result<ResponseOk<JsonProviderSignIn>, HttpError> {
    let ProviderAccount { pin, email, .. } = account;
    let handle = pin.handle.clone();
    let sign_in_context = SignInContext {
        invite: invite_for(context, carried.invite.as_ref(), &email).cloned(),
        pin: Some(pin),
        ..carried
    };
    let emailed = email_sign_in(log, context, request, email.clone(), sign_in_context).await?;
    let (resend_opens, wait, cookie) = match emailed {
        Emailed::Sent(sent) => (resend_after(), None, Some(sent.cookie(context)?)),
        Emailed::Capped { wait, .. } => (None, Some(wait), None),
    };
    let inbox = JsonProviderInbox {
        handle,
        email,
        resend_after: resend_opens,
        wait,
    };
    let mut response = Post::pub_response_ok(JsonProviderSignIn {
        signed_in: None,
        inbox: Some(inbox),
    });
    if let Some(cookie) = cookie {
        response.headers_mut().append(SET_COOKIE, cookie);
    }
    Ok(response)
}
