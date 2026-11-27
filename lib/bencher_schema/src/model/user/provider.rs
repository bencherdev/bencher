//! A sign-in with GitHub or Google: the state row its start makes, which only the browser that
//! started it can finish, once; and the pins that tie a provider account to one user.

use bencher_json::{
    BindingSecret, BindingSecretHash, DateTime, Jwt, NonEmpty, OAuthAuthorizationUuid,
    OrganizationUuid, PkceVerifier, PlanLevel, ProviderAccountId, ProviderNonce, ProviderState,
    ProviderStateHash, SignInProvider,
};
use chrono::TimeDelta;
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, OptionalExtension as _, QueryDsl as _,
    RunQueryDsl as _, SelectableHelper as _,
};
use dropshot::HttpError;
use subtle::ConstantTimeEq as _;

use crate::{
    context::DbConnection,
    error::issue_error,
    model::user::{QueryUser, UserId, session::SessionId, sign_in::SignInContext},
    schema::{self, provider_state as provider_state_table},
};

crate::macros::typed_id::typed_id!(ProviderStateId);

/// A provider's sign-in ends this long after it starts.
pub const PROVIDER_STATE_LIFETIME: TimeDelta = TimeDelta::minutes(10);

#[derive(diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = provider_state_table)]
pub struct QueryProviderState {
    pub id: ProviderStateId,
    /// The session a fresh sign-in proves again, or none for a sign-in.
    pub session_id: Option<SessionId>,
    pub provider: SignInProvider,
    pub state_hash: ProviderStateHash,
    pub binding_hash: BindingSecretHash,
    pub verifier: PkceVerifier,
    /// Google's, which its ID token must carry.
    pub nonce: Option<ProviderNonce>,
    pub invite: Option<Jwt>,
    pub claim: Option<OrganizationUuid>,
    pub plan: Option<PlanLevel>,
    pub authorization: Option<OAuthAuthorizationUuid>,
    pub created: DateTime,
    pub expiration: DateTime,
}

impl QueryProviderState {
    /// End the provider's sign-in that `state` names and return it, if it has not ended and
    /// `binding` is the secret of the browser that started it. Otherwise change nothing.
    pub fn take(
        conn: &mut DbConnection,
        provider: SignInProvider,
        state: &ProviderState,
        binding: Option<&BindingSecret>,
        now: DateTime,
    ) -> diesel::QueryResult<Option<Self>> {
        let Some(row) = schema::provider_state::table
            .filter(schema::provider_state::state_hash.eq(ProviderStateHash::from(state)))
            .filter(schema::provider_state::provider.eq(provider))
            .filter(schema::provider_state::expiration.gt(now))
            .select(Self::as_select())
            .first(conn)
            .optional()?
        else {
            return Ok(None);
        };
        if !binding.is_some_and(|binding| row.is_bound_to(binding)) {
            return Ok(None);
        }
        let ended = diesel::delete(
            schema::provider_state::table.filter(schema::provider_state::id.eq(row.id)),
        )
        .execute(conn)?;
        Ok((ended == 1).then_some(row))
    }

    /// Whether `binding` is the secret of the browser that started the sign-in, compared in
    /// constant time.
    fn is_bound_to(&self, binding: &BindingSecret) -> bool {
        BindingSecretHash::from(binding)
            .as_ref()
            .as_bytes()
            .ct_eq(self.binding_hash.as_ref().as_bytes())
            .into()
    }

    /// What the sign-in carries to where it lands.
    pub fn context(&self) -> SignInContext {
        SignInContext {
            invite: self.invite.clone(),
            claim: self.claim,
            plan: self.plan,
            authorization: self.authorization,
            pin: None,
        }
    }
}

#[derive(diesel::Insertable)]
#[diesel(table_name = provider_state_table)]
pub struct InsertProviderState {
    pub session_id: Option<SessionId>,
    pub provider: SignInProvider,
    pub state_hash: ProviderStateHash,
    pub binding_hash: BindingSecretHash,
    pub verifier: PkceVerifier,
    pub nonce: Option<ProviderNonce>,
    pub invite: Option<Jwt>,
    pub claim: Option<OrganizationUuid>,
    pub plan: Option<PlanLevel>,
    pub authorization: Option<OAuthAuthorizationUuid>,
    pub created: DateTime,
    pub expiration: DateTime,
}

/// The secrets of a provider's new sign-in.
///
/// The row keeps only hashes of the state its round trip carries and of the binding secret its
/// browser's cookie holds, and keeps the verifier and the nonce for the finish.
pub struct ProviderSecrets {
    pub state: ProviderState,
    pub binding: BindingSecret,
    pub verifier: PkceVerifier,
    pub nonce: Option<ProviderNonce>,
}

impl InsertProviderState {
    /// A sign-in with `provider` that starts at `now`, carrying `context`, and its secrets.
    pub fn new(
        provider: SignInProvider,
        context: SignInContext,
        now: DateTime,
    ) -> Result<(Self, ProviderSecrets), HttpError> {
        Self::with_session(provider, context, None, now)
    }

    /// A fresh sign-in with `provider` that proves `session_id` again, starting at `now`, and its
    /// secrets.
    pub fn fresh(
        provider: SignInProvider,
        session_id: SessionId,
        now: DateTime,
    ) -> Result<(Self, ProviderSecrets), HttpError> {
        Self::with_session(provider, SignInContext::default(), Some(session_id), now)
    }

    fn with_session(
        provider: SignInProvider,
        context: SignInContext,
        session_id: Option<SessionId>,
        now: DateTime,
    ) -> Result<(Self, ProviderSecrets), HttpError> {
        let generate_error = |e| {
            issue_error(
                "Failed to generate a sign-in secret",
                "Failed to generate a provider's sign-in secret:",
                e,
            )
        };
        let secrets = ProviderSecrets {
            state: ProviderState::generate().map_err(generate_error)?,
            binding: BindingSecret::generate().map_err(generate_error)?,
            verifier: PkceVerifier::generate().map_err(generate_error)?,
            nonce: match provider {
                SignInProvider::Github => None,
                SignInProvider::Google => Some(ProviderNonce::generate().map_err(generate_error)?),
            },
        };
        let SignInContext {
            invite,
            claim,
            plan,
            authorization,
            pin: _,
        } = context;
        let insert = Self {
            session_id,
            provider,
            state_hash: ProviderStateHash::from(&secrets.state),
            binding_hash: BindingSecretHash::from(&secrets.binding),
            verifier: secrets.verifier.clone(),
            nonce: secrets.nonce.clone(),
            invite,
            claim,
            plan,
            authorization,
            created: now,
            expiration: now + PROVIDER_STATE_LIFETIME,
        };
        Ok((insert, secrets))
    }

    /// Record the sign-in, after dropping every one that has ended.
    pub fn insert(&self, conn: &mut DbConnection, now: DateTime) -> diesel::QueryResult<()> {
        diesel::delete(
            schema::provider_state::table.filter(schema::provider_state::expiration.le(now)),
        )
        .execute(conn)?;
        diesel::insert_into(schema::provider_state::table)
            .values(self)
            .execute(conn)?;
        Ok(())
    }
}

/// A provider account to pin to a user: which provider, the account's ID, and its handle, the
/// GitHub login or the Google email, as its owner knows it.
#[derive(Debug, Clone)]
pub struct ProviderPin {
    pub provider: SignInProvider,
    pub account_id: ProviderAccountId,
    pub handle: NonEmpty,
}

impl ProviderPin {
    /// Each provider that has an account pinned to the user.
    pub fn providers_of(
        conn: &mut DbConnection,
        user_id: UserId,
    ) -> diesel::QueryResult<Vec<SignInProvider>> {
        schema::provider_pin::table
            .filter(schema::provider_pin::user_id.eq(user_id))
            .order(schema::provider_pin::provider)
            .select(schema::provider_pin::provider)
            .load(conn)
    }

    /// The user the account is pinned to, if any.
    pub fn user(&self, conn: &mut DbConnection) -> diesel::QueryResult<Option<QueryUser>> {
        schema::provider_pin::table
            .inner_join(schema::user::table)
            .filter(schema::provider_pin::provider.eq(self.provider))
            .filter(schema::provider_pin::account_id.eq(&self.account_id))
            .select(schema::user::all_columns)
            .first(conn)
            .optional()
    }

    /// Keep the pin's handle the one the provider names now.
    pub fn refresh(&self, conn: &mut DbConnection) -> diesel::QueryResult<usize> {
        diesel::update(
            schema::provider_pin::table
                .filter(schema::provider_pin::provider.eq(self.provider))
                .filter(schema::provider_pin::account_id.eq(&self.account_id)),
        )
        .set(schema::provider_pin::handle.eq(&self.handle))
        .execute(conn)
    }

    /// Pin the account to `user_id` at `now`, in place of the user's pin for this provider, and
    /// return whether it pinned: an account pinned to another user stays as it is.
    pub fn pin(
        &self,
        conn: &mut DbConnection,
        user_id: UserId,
        now: DateTime,
    ) -> diesel::QueryResult<bool> {
        let pinned_elsewhere = schema::provider_pin::table
            .filter(schema::provider_pin::provider.eq(self.provider))
            .filter(schema::provider_pin::account_id.eq(&self.account_id))
            .filter(schema::provider_pin::user_id.ne(user_id))
            .count()
            .get_result::<i64>(conn)?
            > 0;
        if pinned_elsewhere {
            return Ok(false);
        }
        diesel::delete(
            schema::provider_pin::table.filter(
                schema::provider_pin::user_id
                    .eq(user_id)
                    .and(schema::provider_pin::provider.eq(self.provider)),
            ),
        )
        .execute(conn)?;
        diesel::insert_into(schema::provider_pin::table)
            .values((
                schema::provider_pin::user_id.eq(user_id),
                schema::provider_pin::provider.eq(self.provider),
                schema::provider_pin::account_id.eq(&self.account_id),
                schema::provider_pin::handle.eq(&self.handle),
                schema::provider_pin::created.eq(now),
            ))
            .execute(conn)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, ProviderAccountId, SignInProvider};
    use diesel::{QueryDsl as _, RunQueryDsl as _, connection::SimpleConnection as _};
    use pretty_assertions::assert_eq;

    use super::{InsertProviderState, PROVIDER_STATE_LIFETIME, ProviderPin};
    use crate::{
        model::user::{UserId, sign_in::SignInContext},
        schema,
        test_util::setup_test_db,
    };

    // A new sign-in sweeps out every one that has ended, and no other.
    #[test]
    fn insert_drops_ended_provider_sign_ins() {
        let mut conn = setup_test_db();
        let now = DateTime::TEST;
        let started = |at: DateTime| {
            InsertProviderState::new(SignInProvider::Github, SignInContext::default(), at)
                .unwrap()
                .0
        };
        let ended = started(now - PROVIDER_STATE_LIFETIME);
        let live = started(now - PROVIDER_STATE_LIFETIME + chrono::TimeDelta::seconds(1));
        for old in [&ended, &live] {
            diesel::insert_into(schema::provider_state::table)
                .values(old)
                .execute(&mut conn)
                .unwrap();
        }

        started(now).insert(&mut conn, now).unwrap();

        let left = schema::provider_state::table
            .select(schema::provider_state::state_hash)
            .order(schema::provider_state::id)
            .load::<String>(&mut conn)
            .unwrap();
        assert_eq!(left.len(), 2);
        assert_eq!(left.first(), Some(&live.state_hash.to_string()));
    }

    // An account pinned to one user is never pinned to another: that user keeps the pin, and the
    // other is left as it was.
    #[test]
    fn pin_leaves_an_account_pinned_to_another_user() {
        let mut conn = setup_test_db();
        conn.batch_execute(
            "INSERT INTO user (id, uuid, name, slug, email, admin, locked, created, modified)
                VALUES (1, '00000000-0000-4000-8000-000000000001', 'Ada', 'ada', 'ada@example.com', 0, 0, 0, 0),
                    (2, '00000000-0000-4000-8000-000000000002', 'Grace', 'grace', 'grace@example.com', 0, 0, 0, 0);",
        )
        .unwrap();
        let (ada, grace) = (UserId::from_raw(1), UserId::from_raw(2));
        let pin = |account: &str| ProviderPin {
            provider: SignInProvider::Github,
            account_id: account.parse::<ProviderAccountId>().unwrap(),
            handle: "octo-cat".parse().unwrap(),
        };
        let pins = |conn: &mut diesel::SqliteConnection| {
            schema::provider_pin::table
                .select((
                    schema::provider_pin::user_id,
                    schema::provider_pin::account_id,
                ))
                .order(schema::provider_pin::id)
                .load::<(i32, String)>(conn)
                .unwrap()
        };

        assert!(pin("7").pin(&mut conn, ada, DateTime::TEST).unwrap());
        assert!(!pin("7").pin(&mut conn, grace, DateTime::TEST).unwrap());
        assert_eq!(pins(&mut conn), [(1, "7".to_owned())]);
        assert!(pin("8").pin(&mut conn, grace, DateTime::TEST).unwrap());
        assert_eq!(pins(&mut conn), [(1, "7".to_owned()), (2, "8".to_owned())]);
    }
}
