//! Sign in by email: one pending sign-in per browser, proven once by its code or its link.

use bencher_endpoint::{Post, ResponseAccepted};
#[cfg(feature = "plus")]
use bencher_json::NonEmpty;
use bencher_json::{
    BindingSecret, DateTime, Email, JsonSignInEmail, JsonSignedIn, SessionMethod, SessionToken,
    SignInRefusal, UserName,
};
#[cfg(feature = "plus")]
use bencher_schema::model::organization::sso::QuerySso;
use bencher_schema::{
    context::{ApiContext, Body, DbConnection, Message, SignInBody},
    error::{forbidden_error, issue_error, service_unavailable_error, unauthorized_error},
    model::{
        organization::QueryOrganization,
        user::{
            InsertUser, Invited, QueryUser,
            session::{
                InsertSession, QuerySession,
                cookie::{
                    CookieCredential, PendingCookie, SessionCookie, passes_same_origin_check,
                },
            },
            sign_in::{
                InsertPendingSignIn, QueryPendingSignIn, RESEND_WAIT, SignInContext, SignInSecrets,
            },
        },
    },
    public_conn, write_transaction,
};
use dropshot::{HttpError, RequestInfo};
use http::{HeaderValue, header::SET_COOKIE};
use slog::Logger;

pub mod code;
pub mod email;
pub mod link;

/// One message for a pending sign-in that has expired, ended, or never was.
const ENDED: &str = "This sign-in has ended";
const CROSS_ORIGIN: &str = "A sign-in can come only from the console's own origin";
const LOCKED_CLOUD: &str = "This account is locked. To unlock it, contact support.";
const LOCKED_SELF_HOSTED: &str =
    "This account is locked. To unlock it, contact the administrator of this Bencher server.";

/// Admit a request to a sign-in endpoint: from the console's own origin, and charged to the
/// reader's address.
fn admit(log: &Logger, context: &ApiContext, request: &RequestInfo) -> Result<(), HttpError> {
    check_same_origin(context, request)?;
    #[cfg(feature = "plus")]
    charge_attempt(context, context.remote_ip(log, request.headers()))?;
    #[cfg(not(feature = "plus"))]
    let _ = log;
    Ok(())
}

/// [`admit`] a request that may send an email, then pass it through Turnstile, where the server
/// has it.
async fn admit_email(
    log: &Logger,
    context: &ApiContext,
    request: &RequestInfo,
    #[cfg(feature = "plus")] turnstile: Option<&NonEmpty>,
) -> Result<(), HttpError> {
    check_same_origin(context, request)?;
    #[cfg(feature = "plus")]
    {
        let remote_ip = context.remote_ip(log, request.headers());
        charge_attempt(context, remote_ip)?;
        if let Some(verifier) = &context.turnstile
            && let Err(_err) = verifier.verify(log, turnstile, remote_ip).await
        {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::SignInRefused(
                bencher_otel::SignInRefusalReason::TurnstileFailed,
            ));
            return Err(refused(
                forbidden_error("Turnstile verification failed"),
                SignInRefusal::TurnstileFailed,
            ));
        }
    }
    #[cfg(not(feature = "plus"))]
    let _ = log;
    Ok(())
}

pub(crate) fn check_same_origin(
    context: &ApiContext,
    request: &RequestInfo,
) -> Result<(), HttpError> {
    if passes_same_origin_check(request.method(), request.headers(), &context.console_url) {
        Ok(())
    } else {
        Err(refused(
            forbidden_error(CROSS_ORIGIN),
            SignInRefusal::CrossOrigin,
        ))
    }
}

#[cfg(feature = "plus")]
fn charge_attempt(
    context: &ApiContext,
    remote_ip: Option<std::net::IpAddr>,
) -> Result<(), HttpError> {
    if let Some(remote_ip) = remote_ip {
        context
            .rate_limiting
            .public_auth_attempt(remote_ip)
            .map_err(|error| refused(error, SignInRefusal::TooManyAttempts))?;
    }
    Ok(())
}

/// The error, with the refusal as its code.
fn refused(mut error: HttpError, refusal: SignInRefusal) -> HttpError {
    error.error_code = Some(refusal.as_ref().to_owned());
    error
}

/// The binding secret of the request's pending cookie, if the API honors it.
fn binding(context: &ApiContext, request: &RequestInfo) -> Option<BindingSecret> {
    PendingCookie::new(&context.console_url).binding(request.headers(), &context.relay)
}

fn ended() -> HttpError {
    refused(unauthorized_error(ENDED), SignInRefusal::SignInEnded)
}

/// Count an email to `email`, unless the address is at its cap: then the seconds until it is not.
fn at_cap(context: &ApiContext, email: &Email) -> Option<u32> {
    #[cfg(feature = "plus")]
    if let Err(wait) = context.rate_limiting.sign_in_email(email) {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::SignInRefused(
            bencher_otel::SignInRefusalReason::AddressCapped,
        ));
        return Some(u32::try_from(wait.as_secs()).unwrap_or(u32::MAX));
    }
    #[cfg(not(feature = "plus"))]
    let _ = (context, email);
    None
}

/// A pending sign-in that has started: its row and its secrets.
struct Start {
    insert: InsertPendingSignIn,
    secrets: SignInSecrets,
}

impl Start {
    /// Record a new pending sign-in for `email`.
    async fn insert(
        context: &ApiContext,
        email: Email,
        sign_in_context: SignInContext,
        now: DateTime,
    ) -> Result<Self, HttpError> {
        let (insert, secrets) =
            InsertPendingSignIn::new(&context.code_key, email, sign_in_context, now)?;
        write_transaction!(context, |conn| insert.insert(conn, now)).map_err(|e| {
            issue_error("Failed to start a sign-in", "Failed to start a sign-in:", e)
        })?;
        Ok(Self { insert, secrets })
    }

    /// Email its code and link.
    async fn send(self, log: &Logger, context: &ApiContext) -> Result<Sent, HttpError> {
        let Self { insert, secrets } = self;
        let SignInSecrets {
            binding,
            link,
            code,
        } = secrets;
        let email = insert.email;
        let query_user =
            QueryUser::find_with_email(public_conn!(context), &email).map_err(|e| {
                issue_error(
                    "Failed to look up a user",
                    "Failed to look up a user to sign in:",
                    e,
                )
            })?;
        let body = if let Some(query_user) = &query_user
            && query_user.locked
        {
            SignInBody::Locked {
                support: support_url(context),
            }
        } else {
            let mut link_url = context.console_url.join("/auth/confirm").map_err(|e| {
                issue_error(
                    "Failed to make a sign-in link",
                    "Failed to make a sign-in link:",
                    e,
                )
            })?;
            link_url.set_fragment(Some(link.as_ref()));
            SignInBody::Code {
                sign_up: query_user.is_none(),
                code,
                link: link_url,
            }
        };
        let message = Message {
            to_name: None,
            to_email: email.clone().into(),
            subject: Some(body.subject()),
            body: Some(Body::SignIn(body)),
        };
        context
            .messenger
            .send_and_wait(log, message)
            .await
            .map_err(|e| {
                slog::error!(log, "Failed to send a sign-in email: {e}");
                refused(service_unavailable_error(e), SignInRefusal::EmailFailed)
            })?;
        Ok(Sent { email, binding })
    }
}

/// A pending sign-in whose email went out.
struct Sent {
    email: Email,
    binding: BindingSecret,
}

impl Sent {
    /// End it, so that neither its code nor its link works.
    fn end(&self, conn: &mut DbConnection) -> diesel::QueryResult<usize> {
        QueryPendingSignIn::end_binding(conn, &self.binding)
    }

    /// Answer with the cookie that binds it to this browser.
    fn answer(self, context: &ApiContext) -> Result<ResponseAccepted<JsonSignInEmail>, HttpError> {
        let Self { email, binding } = self;
        let cookie = PendingCookie::new(&context.console_url)
            .set(&binding)
            .map_err(|e| {
                issue_error(
                    "Failed to set the pending sign-in cookie",
                    "Failed to set the pending sign-in cookie:",
                    e,
                )
            })?;
        let mut response = Post::pub_response_accepted(JsonSignInEmail {
            email,
            resend_after: u32::try_from(RESEND_WAIT.num_seconds()).ok(),
            wait: None,
        });
        response.headers_mut().append(SET_COOKIE, cookie);
        Ok(response)
    }
}

/// End the pending sign-in a new one replaces, in one write transaction, once the new one's email
/// went out, and return how many `end` ended.
/// Should the write fail, the replaced one lives only until it expires, so the failure is reported
/// and the new sign-in still answers.
async fn end_replaced<F>(context: &ApiContext, end: F) -> Option<usize>
where
    F: FnOnce(&mut DbConnection) -> diesel::QueryResult<usize>,
{
    match write_transaction!(context, |conn| end(conn)) {
        Ok(ended) => Some(ended),
        Err(e) => {
            let _err = issue_error(
                "Failed to end a replaced sign-in",
                "Failed to end the pending sign-in a new one replaces:",
                e,
            );
            None
        },
    }
}

/// The answer for an address at its cap, which sends nothing and changes nothing.
fn capped(email: Email, wait: u32) -> ResponseAccepted<JsonSignInEmail> {
    Post::pub_response_accepted(JsonSignInEmail {
        email,
        resend_after: None,
        wait: Some(wait),
    })
}

/// Where a locked account reaches support, on Bencher Cloud.
fn support_url(context: &ApiContext) -> Option<url::Url> {
    #[cfg(feature = "plus")]
    if context.is_bencher_cloud {
        return context.console_url.join("/help").ok();
    }
    #[cfg(not(feature = "plus"))]
    let _ = context;
    None
}

/// What the browser presents at a proof: its device, and any session it is signed in to.
pub(crate) struct Presented<'a> {
    pub user_agent: Option<&'a str>,
    pub session: Option<SessionToken>,
}

impl<'a> Presented<'a> {
    pub(crate) fn new(context: &ApiContext, request: &'a RequestInfo) -> Self {
        Self {
            user_agent: request
                .headers()
                .get(http::header::USER_AGENT)
                .and_then(|user_agent| user_agent.to_str().ok()),
            session: CookieCredential::from_request(request).and_then(|cookie| {
                cookie
                    .honored(&context.console_url, &context.relay)?
                    .parse()
                    .ok()
            }),
        }
    }
}

/// What a proof did: signed in, or found the account locked.
enum Proof {
    SignedIn(Box<SignedIn>),
    Locked,
}

struct SignedIn {
    user: QueryUser,
    /// The account, if the proof made it.
    new: Option<InsertUser>,
    /// Whether the user joined the invite's organization.
    invited: bool,
    /// Whether the user claimed the organization.
    claimed: bool,
    #[cfg(feature = "plus")]
    sso_joined: bool,
    token: SessionToken,
    context: SignInContext,
}

/// An error inside a sign-in's write transaction, which rolls the whole sign-in back.
enum TxError {
    Db(diesel::result::Error),
    Http(HttpError),
}

impl From<diesel::result::Error> for TxError {
    fn from(error: diesel::result::Error) -> Self {
        Self::Db(error)
    }
}

impl From<HttpError> for TxError {
    fn from(error: HttpError) -> Self {
        Self::Http(error)
    }
}

impl TxError {
    fn into_http(self) -> HttpError {
        match self {
            Self::Db(e) => issue_error("Failed to sign in", "Failed to sign in:", e),
            Self::Http(e) => e,
        }
    }
}

/// Prove the pending sign-in, inside its write transaction: end it, so neither its code nor its
/// link works again, then sign in the address's account, or make one, and begin a session in
/// place of any the browser presented.
fn prove(
    conn: &mut DbConnection,
    context: &ApiContext,
    pending: &QueryPendingSignIn,
    presented: &Presented<'_>,
    now: DateTime,
) -> Result<Proof, TxError> {
    QueryPendingSignIn::end(conn, pending.id)?;
    let email = &pending.email;
    let (user, new) = if let Some(user) = QueryUser::find_with_email(conn, email)? {
        if user.locked {
            return Ok(Proof::Locked);
        }
        (user, None)
    } else {
        let (insert_user, user) =
            InsertUser::sign_up(conn, UserName::from_email(email), None, email.clone())?;
        (user, Some(insert_user))
    };
    let (invited, claimed) = join(conn, context, pending, &user, now)?;
    if new.is_some() && !invited && !claimed {
        InsertUser::personal_organization(conn, &user)?;
    }
    QueryUser::verify_email(conn, user.id, email, now)?;
    #[cfg(feature = "plus")]
    let sso_joined = QuerySso::join_all_with(conn, &user, now)?;
    if let Some(token) = &presented.session {
        QuerySession::delete_by_token(conn, token)?;
    }
    let (session, token) =
        InsertSession::new(user.id, SessionMethod::Email, presented.user_agent, now)?;
    session.insert(conn, now)?;
    Ok(Proof::SignedIn(Box::new(SignedIn {
        user,
        new,
        invited,
        claimed,
        #[cfg(feature = "plus")]
        sso_joined,
        token,
        context: pending.context(),
    })))
}

/// Join the organization the sign-in's invite or claim names, unless that no longer applies:
/// the organization is gone, the user already belongs to it, or someone else claimed it.
/// An invite, applied or not, keeps the claim from being considered.
/// Returns whether the user joined by the invite, and whether by the claim.
fn join(
    conn: &mut DbConnection,
    context: &ApiContext,
    pending: &QueryPendingSignIn,
    user: &QueryUser,
    now: DateTime,
) -> Result<(bool, bool), TxError> {
    if let Some(invite) = &pending.invite {
        let invited = user.accept_invite_with(conn, &context.token_key, invite, now)?;
        return Ok((invited == Invited::Joined, false));
    }
    if let Some(claim) = pending.claim
        && let Some(organization) = QueryOrganization::find(conn, claim)?
    {
        let claimed = organization.claim_with(conn, user.id, now)?.is_some();
        return Ok((false, claimed));
    }
    Ok((false, false))
}

/// Finish a proven sign-in: notify the admins of a new account, and answer with the session's
/// cookie in place of the pending one.
async fn finish(
    log: &Logger,
    context: &ApiContext,
    proof: Proof,
) -> Result<(JsonSignedIn, [HeaderValue; 2]), HttpError> {
    let SignedIn {
        user,
        new,
        invited,
        claimed,
        #[cfg(feature = "plus")]
        sso_joined,
        token,
        context: sign_in_context,
    } = match proof {
        Proof::SignedIn(signed_in) => *signed_in,
        Proof::Locked => return Err(locked(context)),
    };

    if let Some(insert_user) = &new {
        let notices = insert_user.admin_notices(
            public_conn!(context),
            &context.console_url,
            invited,
            "email",
        )?;
        for notice in notices {
            if let Err(e) = context.messenger.send_and_wait(log, notice).await {
                slog::error!(log, "Failed to notify an admin of a new user: {e}");
            }
        }
    }

    #[cfg(feature = "otel")]
    {
        let auth_method = bencher_otel::AuthMethod::Email;
        bencher_otel::ApiMeter::increment(if new.is_some() {
            bencher_otel::ApiCounter::UserSignup(auth_method)
        } else {
            bencher_otel::ApiCounter::UserLogin(auth_method)
        });
        if invited && new.is_none() {
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserAccept(Some(
                auth_method,
            )));
        }
        if claimed {
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserClaim);
        }
        #[cfg(feature = "plus")]
        if sso_joined {
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserSsoJoin(auth_method));
        }
    }
    #[cfg(not(feature = "otel"))]
    let _ = claimed;
    #[cfg(all(feature = "plus", not(feature = "otel")))]
    let _ = sso_joined;

    let session_cookie = SessionCookie::new(&context.console_url)
        .set(&token)
        .map_err(|e| {
            issue_error(
                "Failed to set the session cookie",
                "Failed to set the session cookie:",
                e,
            )
        })?;
    let pending_cookie = PendingCookie::new(&context.console_url).clear();
    let SignInContext {
        invite: _,
        claim: _,
        #[cfg(feature = "plus")]
        plan,
        #[cfg(feature = "plus")]
        authorization,
    } = sign_in_context;
    let json = JsonSignedIn {
        user: user.into_json(),
        new: new.is_some(),
        #[cfg(feature = "plus")]
        plan,
        #[cfg(feature = "plus")]
        authorization,
    };
    Ok((json, [session_cookie, pending_cookie]))
}

/// The refusal of a locked account's proof, which has ended its pending sign-in.
fn locked(context: &ApiContext) -> HttpError {
    let message = if support_url(context).is_some() {
        LOCKED_CLOUD
    } else {
        LOCKED_SELF_HOSTED
    };
    let mut error = refused(forbidden_error(message), SignInRefusal::Locked);
    error
        .headers_mut()
        .append(SET_COOKIE, PendingCookie::new(&context.console_url).clear());
    error
}
