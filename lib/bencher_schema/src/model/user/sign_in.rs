//! A pending sign-in by email: one per browser, proven once, by its code or its link.

use aws_lc_rs::{
    error::Unspecified,
    hkdf::{HKDF_SHA256, Salt},
    hmac,
};
use bencher_json::{
    BindingSecret, BindingSecretHash, DateTime, Email, EmailHash, Jwt, LinkSecret, LinkSecretHash,
    OrganizationUuid, PendingSignInUuid, Secret, SignInCode, SignInCodeHash,
};
#[cfg(feature = "plus")]
use bencher_json::{
    NonEmpty, OAuthAuthorizationUuid, PlanLevel, ProviderAccountId, SignInProvider,
};
use chrono::TimeDelta;
use diesel::{
    ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _,
    SelectableHelper as _,
};
use dropshot::HttpError;
use subtle::ConstantTimeEq as _;

#[cfg(feature = "plus")]
use crate::model::user::provider::ProviderPin;
use crate::{
    context::DbConnection,
    error::issue_error,
    model::user::session::SessionId,
    schema::{self, pending_sign_in as pending_sign_in_table},
};

crate::macros::typed_id::typed_id!(PendingSignInId);

/// A pending sign-in ends this long after it starts.
pub const PENDING_SIGN_IN_LIFETIME: TimeDelta = TimeDelta::minutes(30);
/// Resend opens this long after a pending sign-in starts.
pub const RESEND_WAIT: TimeDelta = TimeDelta::seconds(60);
/// The wrong codes that end a pending sign-in.
pub const CODE_TRIES: i32 = 5;
/// The wrong codes an address may make in [`CODE_BUDGET_WINDOW`], across all of its sign-ins,
/// before its codes pause.
pub const CODE_BUDGET: i64 = 10;
/// How long a wrong code counts against its address's budget.
pub const CODE_BUDGET_WINDOW: TimeDelta = TimeDelta::hours(24);

#[derive(Clone, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = pending_sign_in_table)]
pub struct QueryPendingSignIn {
    pub id: PendingSignInId,
    pub uuid: PendingSignInUuid,
    /// The session a fresh sign-in proves again, or none for a sign-in.
    pub session_id: Option<SessionId>,
    pub email: Email,
    pub binding_hash: BindingSecretHash,
    pub code_hash: SignInCodeHash,
    pub link_hash: LinkSecretHash,
    /// The wrong codes tried so far.
    pub tries: i32,
    pub invite: Option<Jwt>,
    pub claim: Option<OrganizationUuid>,
    #[cfg(feature = "plus")]
    pub plan: Option<PlanLevel>,
    #[cfg(feature = "plus")]
    pub authorization: Option<OAuthAuthorizationUuid>,
    #[cfg(feature = "plus")]
    pub pin_provider: Option<SignInProvider>,
    #[cfg(feature = "plus")]
    pub pin_account: Option<ProviderAccountId>,
    #[cfg(feature = "plus")]
    pub pin_handle: Option<NonEmpty>,
    pub created: DateTime,
    pub expiration: DateTime,
}

impl QueryPendingSignIn {
    /// The pending sign-in a browser's binding secret names, unless it has ended or is a fresh
    /// sign-in.
    pub fn from_binding(
        conn: &mut DbConnection,
        binding: &BindingSecret,
        now: DateTime,
    ) -> diesel::QueryResult<Option<Self>> {
        schema::pending_sign_in::table
            .filter(schema::pending_sign_in::binding_hash.eq(BindingSecretHash::from(binding)))
            .filter(schema::pending_sign_in::session_id.is_null())
            .filter(schema::pending_sign_in::expiration.gt(now))
            .select(Self::as_select())
            .first(conn)
            .optional()
    }

    /// The fresh sign-in a browser's binding secret names for `session_id`, unless it has ended.
    pub fn fresh_from_binding(
        conn: &mut DbConnection,
        binding: &BindingSecret,
        session_id: SessionId,
        now: DateTime,
    ) -> diesel::QueryResult<Option<Self>> {
        schema::pending_sign_in::table
            .filter(schema::pending_sign_in::binding_hash.eq(BindingSecretHash::from(binding)))
            .filter(schema::pending_sign_in::session_id.eq(session_id))
            .filter(schema::pending_sign_in::expiration.gt(now))
            .select(Self::as_select())
            .first(conn)
            .optional()
    }

    /// The pending sign-in a link's secret names, a fresh one included, unless it has ended.
    pub fn from_link(
        conn: &mut DbConnection,
        link: &LinkSecret,
        now: DateTime,
    ) -> diesel::QueryResult<Option<Self>> {
        schema::pending_sign_in::table
            .filter(schema::pending_sign_in::link_hash.eq(LinkSecretHash::from(link)))
            .filter(schema::pending_sign_in::expiration.gt(now))
            .select(Self::as_select())
            .first(conn)
            .optional()
    }

    /// End the pending sign-in, so that neither its code nor its link works again.
    pub fn end(conn: &mut DbConnection, id: PendingSignInId) -> diesel::QueryResult<usize> {
        diesel::delete(schema::pending_sign_in::table.filter(schema::pending_sign_in::id.eq(id)))
            .execute(conn)
    }

    /// End the pending sign-in a browser's binding secret names, if any.
    pub fn end_binding(
        conn: &mut DbConnection,
        binding: &BindingSecret,
    ) -> diesel::QueryResult<usize> {
        diesel::delete(
            schema::pending_sign_in::table
                .filter(schema::pending_sign_in::binding_hash.eq(BindingSecretHash::from(binding))),
        )
        .execute(conn)
    }

    /// Count a wrong code, ending the pending sign-in at its last try, and return the tries left.
    pub fn count_wrong_code(&self, conn: &mut DbConnection) -> diesel::QueryResult<i32> {
        let tries = self.tries.saturating_add(1);
        if tries < CODE_TRIES {
            diesel::update(
                schema::pending_sign_in::table.filter(schema::pending_sign_in::id.eq(self.id)),
            )
            .set(schema::pending_sign_in::tries.eq(tries))
            .execute(conn)?;
        } else {
            Self::end(conn, self.id)?;
        }
        Ok(CODE_TRIES.saturating_sub(tries).max(0))
    }

    /// When Resend opens.
    pub fn resend_opens(&self) -> DateTime {
        self.created + RESEND_WAIT
    }

    /// Another pending sign-in for its address, starting at `now`, that carries what it carries
    /// or proves again the session it does, and its secrets.
    pub fn again(
        &self,
        code_key: &CodeKey,
        now: DateTime,
    ) -> Result<(InsertPendingSignIn, SignInSecrets), HttpError> {
        InsertPendingSignIn::with_session(
            code_key,
            self.email.clone(),
            self.context(),
            self.session_id,
            now,
        )
    }

    pub fn context(&self) -> SignInContext {
        SignInContext {
            invite: self.invite.clone(),
            claim: self.claim,
            #[cfg(feature = "plus")]
            plan: self.plan,
            #[cfg(feature = "plus")]
            authorization: self.authorization,
            #[cfg(feature = "plus")]
            pin: self.pin(),
        }
    }

    /// The provider account a proof pins in the asking browser, if a provider started the sign-in.
    #[cfg(feature = "plus")]
    pub fn pin(&self) -> Option<ProviderPin> {
        Some(ProviderPin {
            provider: self.pin_provider?,
            account_id: self.pin_account.clone()?,
            handle: self.pin_handle.clone()?,
        })
    }
}

/// What a pending sign-in carries from its start to its proof.
#[derive(Clone, Default)]
pub struct SignInContext {
    pub invite: Option<Jwt>,
    pub claim: Option<OrganizationUuid>,
    #[cfg(feature = "plus")]
    pub plan: Option<PlanLevel>,
    #[cfg(feature = "plus")]
    pub authorization: Option<OAuthAuthorizationUuid>,
    /// A provider account that the proof pins, in place of the user's pin for that provider.
    #[cfg(feature = "plus")]
    pub pin: Option<ProviderPin>,
}

#[derive(diesel::Insertable)]
#[diesel(table_name = pending_sign_in_table)]
pub struct InsertPendingSignIn {
    pub uuid: PendingSignInUuid,
    pub session_id: Option<SessionId>,
    pub email: Email,
    pub binding_hash: BindingSecretHash,
    pub code_hash: SignInCodeHash,
    pub link_hash: LinkSecretHash,
    pub tries: i32,
    pub invite: Option<Jwt>,
    pub claim: Option<OrganizationUuid>,
    #[cfg(feature = "plus")]
    pub plan: Option<PlanLevel>,
    #[cfg(feature = "plus")]
    pub authorization: Option<OAuthAuthorizationUuid>,
    #[cfg(feature = "plus")]
    pub pin_provider: Option<SignInProvider>,
    #[cfg(feature = "plus")]
    pub pin_account: Option<ProviderAccountId>,
    #[cfg(feature = "plus")]
    pub pin_handle: Option<NonEmpty>,
    pub created: DateTime,
    pub expiration: DateTime,
}

/// The secrets of a new pending sign-in, which no row holds.
pub struct SignInSecrets {
    /// For the browser's pending cookie.
    pub binding: BindingSecret,
    /// For the email's link.
    pub link: LinkSecret,
    /// For the email.
    pub code: SignInCode,
}

impl InsertPendingSignIn {
    /// A pending sign-in for `email` that starts at `now`, and its secrets.
    pub fn new(
        code_key: &CodeKey,
        email: Email,
        context: SignInContext,
        now: DateTime,
    ) -> Result<(Self, SignInSecrets), HttpError> {
        Self::with_session(code_key, email, context, None, now)
    }

    /// A fresh sign-in that proves `session_id` again by an email to `email`, starting at `now`,
    /// and its secrets.
    pub fn fresh(
        code_key: &CodeKey,
        email: Email,
        session_id: SessionId,
        now: DateTime,
    ) -> Result<(Self, SignInSecrets), HttpError> {
        Self::with_session(
            code_key,
            email,
            SignInContext::default(),
            Some(session_id),
            now,
        )
    }

    fn with_session(
        code_key: &CodeKey,
        email: Email,
        context: SignInContext,
        session_id: Option<SessionId>,
        now: DateTime,
    ) -> Result<(Self, SignInSecrets), HttpError> {
        let generate_error = |e| {
            issue_error(
                "Failed to generate a sign-in secret",
                "Failed to generate a sign-in secret:",
                e,
            )
        };
        let secrets = SignInSecrets {
            binding: BindingSecret::generate().map_err(generate_error)?,
            link: LinkSecret::generate().map_err(generate_error)?,
            code: SignInCode::generate(),
        };
        let SignInContext {
            invite,
            claim,
            #[cfg(feature = "plus")]
            plan,
            #[cfg(feature = "plus")]
            authorization,
            #[cfg(feature = "plus")]
            pin,
        } = context;
        #[cfg(feature = "plus")]
        let (pin_provider, pin_account, pin_handle) = pin.map_or((None, None, None), |pin| {
            let ProviderPin {
                provider,
                account_id,
                handle,
            } = pin;
            (Some(provider), Some(account_id), Some(handle))
        });
        let pending_sign_in = Self {
            uuid: PendingSignInUuid::new(),
            session_id,
            email,
            binding_hash: BindingSecretHash::from(&secrets.binding),
            code_hash: code_key.hash(&secrets.code),
            link_hash: LinkSecretHash::from(&secrets.link),
            tries: 0,
            invite,
            claim,
            #[cfg(feature = "plus")]
            plan,
            #[cfg(feature = "plus")]
            authorization,
            #[cfg(feature = "plus")]
            pin_provider,
            #[cfg(feature = "plus")]
            pin_account,
            #[cfg(feature = "plus")]
            pin_handle,
            created: now,
            expiration: now + PENDING_SIGN_IN_LIFETIME,
        };
        Ok((pending_sign_in, secrets))
    }

    /// Record the pending sign-in, after dropping every one that has ended.
    pub fn insert(&self, conn: &mut DbConnection, now: DateTime) -> diesel::QueryResult<()> {
        diesel::delete(
            schema::pending_sign_in::table.filter(schema::pending_sign_in::expiration.le(now)),
        )
        .execute(conn)?;
        diesel::insert_into(schema::pending_sign_in::table)
            .values(self)
            .execute(conn)?;
        Ok(())
    }
}

/// The budget of wrong codes for each address.
///
/// It holds on every server, whatever its rate limits. Each wrong code is a row that holds only
/// the hash of its address, and counts for [`CODE_BUDGET_WINDOW`] after it was made.
pub struct CodeBudget;

impl CodeBudget {
    /// When codes for `email` resume, if its budget is spent at `now`: once the oldest of the
    /// last [`CODE_BUDGET`] wrong codes leaves the window.
    pub fn paused_until(
        conn: &mut DbConnection,
        email: &Email,
        now: DateTime,
    ) -> diesel::QueryResult<Option<DateTime>> {
        let last_in_budget = schema::wrong_code::table
            .filter(schema::wrong_code::email_hash.eq(EmailHash::from(email)))
            .filter(schema::wrong_code::created.gt(now - CODE_BUDGET_WINDOW))
            .order(schema::wrong_code::created.desc())
            .select(schema::wrong_code::created)
            .offset(CODE_BUDGET.saturating_sub(1))
            .first::<DateTime>(conn)
            .optional()?;
        Ok(last_in_budget.map(|created| created + CODE_BUDGET_WINDOW))
    }

    /// Spend one of `email`'s budget on a wrong code, after dropping every wrong code that has
    /// left the window.
    pub fn spend(conn: &mut DbConnection, email: &Email, now: DateTime) -> diesel::QueryResult<()> {
        diesel::delete(
            schema::wrong_code::table
                .filter(schema::wrong_code::created.le(now - CODE_BUDGET_WINDOW)),
        )
        .execute(conn)?;
        diesel::insert_into(schema::wrong_code::table)
            .values((
                schema::wrong_code::email_hash.eq(EmailHash::from(email)),
                schema::wrong_code::created.eq(now),
            ))
            .execute(conn)?;
        Ok(())
    }
}

/// The key a sign-in code is hashed under, derived for that alone from the server's secret key,
/// since a 6-digit code under a plain hash is a table of a million entries.
pub struct CodeKey(hmac::Key);

#[derive(Debug, thiserror::Error)]
#[error("Failed to derive the sign-in code key: {0}")]
pub struct CodeKeyError(Unspecified);

/// A different info string derives a different key, under which no stored code matches.
const CODE_KEY_INFO: &[u8] = b"bencher.sign_in.code.v1";

impl CodeKey {
    pub fn new(secret_key: &Secret) -> Result<Self, CodeKeyError> {
        let prk = Salt::none(HKDF_SHA256).extract(secret_key.as_ref().as_bytes());
        let okm = prk
            .expand(&[CODE_KEY_INFO], hmac::HMAC_SHA256)
            .map_err(CodeKeyError)?;
        Ok(Self(hmac::Key::from(okm)))
    }

    pub fn hash(&self, code: &SignInCode) -> SignInCodeHash {
        SignInCodeHash::from_digest(hmac::sign(&self.0, code.as_ref().as_bytes()).as_ref())
    }

    /// Whether `code` is the one `hash` was made from, compared in constant time.
    pub fn matches(&self, code: &SignInCode, hash: &SignInCodeHash) -> bool {
        self.hash(code)
            .as_ref()
            .as_bytes()
            .ct_eq(hash.as_ref().as_bytes())
            .into()
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, Email, EmailHash, Secret, SignInCode};
    use diesel::{QueryDsl as _, RunQueryDsl as _};
    use pretty_assertions::assert_eq;

    use super::{
        CODE_BUDGET_WINDOW, CodeBudget, CodeKey, InsertPendingSignIn, PENDING_SIGN_IN_LIFETIME,
        QueryPendingSignIn, SignInContext,
    };
    use crate::{schema, test_util::setup_test_db};

    fn code_key(secret: &str) -> CodeKey {
        CodeKey::new(&secret.parse::<Secret>().unwrap()).unwrap()
    }

    // A code's hash needs the server's secret key, so a read of the database yields no code.
    #[test]
    fn code_hash_is_keyed_by_the_secret_key() {
        let code: SignInCode = "123456".parse().unwrap();
        let key = code_key("a-secret-key");
        let hash = key.hash(&code);
        assert!(key.matches(&code, &hash));
        assert!(!key.matches(&"123457".parse().unwrap(), &hash));
        assert!(!code_key("another-secret-key").matches(&code, &hash));
        assert_ne!(
            hash.as_ref(),
            "8d969eef6ecad3c29a3a629280e686cf0c3f5d5a86aff3ca12020c923adc6c92",
            "not the plain SHA-256 of the code"
        );
    }

    // A new pending sign-in sweeps out every one that has ended, and no other.
    #[test]
    fn insert_drops_ended_pending_sign_ins() {
        let mut conn = setup_test_db();
        let key = code_key("a-secret-key");
        let now = DateTime::TEST;
        let pending = |started: DateTime| {
            let (pending, _secrets) = InsertPendingSignIn::new(
                &key,
                "ada@example.com".parse().unwrap(),
                SignInContext::default(),
                started,
            )
            .unwrap();
            pending
        };
        let ended = pending(now - PENDING_SIGN_IN_LIFETIME);
        let live = pending(now - PENDING_SIGN_IN_LIFETIME + chrono::TimeDelta::seconds(1));
        for old in [&ended, &live] {
            diesel::insert_into(schema::pending_sign_in::table)
                .values(old)
                .execute(&mut conn)
                .unwrap();
        }

        let new = pending(now);
        new.insert(&mut conn, now).unwrap();

        let left = schema::pending_sign_in::table
            .select(schema::pending_sign_in::uuid)
            .order(schema::pending_sign_in::id)
            .load::<bencher_json::PendingSignInUuid>(&mut conn)
            .unwrap();
        assert_eq!(left, vec![live.uuid, new.uuid]);
    }

    // A pending sign-in is found by its secrets only until it ends, at its exact limit.
    #[test]
    fn pending_sign_in_ends_at_its_lifetime() {
        let mut conn = setup_test_db();
        let key = code_key("a-secret-key");
        let start = DateTime::TEST;
        let (pending, secrets) = InsertPendingSignIn::new(
            &key,
            "ada@example.com".parse().unwrap(),
            SignInContext::default(),
            start,
        )
        .unwrap();
        pending.insert(&mut conn, start).unwrap();

        let last = start + PENDING_SIGN_IN_LIFETIME - chrono::TimeDelta::seconds(1);
        let end = start + PENDING_SIGN_IN_LIFETIME;
        for (now, found) in [(last, true), (end, false)] {
            assert_eq!(
                QueryPendingSignIn::from_binding(&mut conn, &secrets.binding, now)
                    .unwrap()
                    .is_some(),
                found
            );
            assert_eq!(
                QueryPendingSignIn::from_link(&mut conn, &secrets.link, now)
                    .unwrap()
                    .is_some(),
                found
            );
        }
    }

    // A wrong code sweeps out every one that has left the window, of every address, and no other.
    #[test]
    fn spend_drops_wrong_codes_that_left_the_window() {
        let mut conn = setup_test_db();
        let now = DateTime::TEST;
        let ada: Email = "ada@example.com".parse().unwrap();
        let grace: Email = "grace@example.com".parse().unwrap();
        let left = now - CODE_BUDGET_WINDOW;
        let counting = left + chrono::TimeDelta::seconds(1);
        CodeBudget::spend(&mut conn, &grace, left).unwrap();
        CodeBudget::spend(&mut conn, &grace, counting).unwrap();
        CodeBudget::spend(&mut conn, &ada, now).unwrap();

        let kept = schema::wrong_code::table
            .select((schema::wrong_code::email_hash, schema::wrong_code::created))
            .order(schema::wrong_code::id)
            .load::<(EmailHash, DateTime)>(&mut conn)
            .unwrap();
        assert_eq!(
            kept,
            vec![
                (EmailHash::from(&grace), counting),
                (EmailHash::from(&ada), now)
            ]
        );
    }
}
