use std::str::FromStr as _;
use std::sync::LazyLock;

use bencher_json::{
    DateTime, Email, Jwt, OrganizationUuid, Secret, organization::member::OrganizationRole,
    system::config::JsonPreviousSecretKey,
};
use chrono::Utc;
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode,
    errors::ErrorKind as JsonWebTokenErrorKind,
};
use serde::de::DeserializeOwned;

#[cfg(feature = "plus")]
use bencher_json::ProjectUuid;
#[cfg(feature = "plus")]
use bencher_json::RunnerUuid;

#[cfg(feature = "plus")]
use crate::ProjectOciClaims;
#[cfg(feature = "plus")]
use crate::RunnerOciClaims;
#[cfg(feature = "plus")]
use crate::claims::ProjectOciTokenClaims;
#[cfg(feature = "plus")]
use crate::claims::RunnerOciTokenClaims;
use crate::claims::{HasIat, PublicOciTokenClaims};
use crate::{
    Audience, AuthOciClaims, Claims, InviteClaims, OAuthClaims, OciAction, OciScopeClaims,
    OrgClaims, PublicOciClaims, StateClaims, TokenError,
};

static HEADER: LazyLock<Header> = LazyLock::new(Header::default);
const ALGORITHM: Algorithm = Algorithm::HS256;

pub struct TokenKey {
    issuer: String,
    encoding: EncodingKey,
    decoding: DecodingKey,
    previous: Vec<PreviousKey>,
}

/// Which key inside a `TokenKey` validated a given JWT.
///
/// `auth.rs` consumes this to apply the compromise-resistant DB-creation
/// check on `ApiKey` tokens when they validate against a previous key.
#[derive(Debug, Clone, Copy)]
pub enum KeyMatch {
    Current,
    Previous {
        /// Earliest valid creation timestamp for this key.
        creation: DateTime,
        /// Last valid creation timestamp for this key.
        retired: DateTime,
    },
}

struct PreviousKey {
    decoding: DecodingKey,
    creation: DateTime,
    retired: DateTime,
}

impl TokenKey {
    pub fn new(issuer: String, secret_key: &Secret) -> Self {
        Self::new_with_previous(issuer, secret_key, &[])
    }

    /// Construct a [`TokenKey`] that signs with `secret_key` and also validates
    /// JWTs signed by any key in `previous` whose active window contains the
    /// token's `iat`.
    ///
    /// # Rotation procedure
    ///
    /// 1. Generate a new high-entropy secret.
    /// 2. In `bencher.json`: move the existing `secret_key` into
    ///    `previous_secret_keys` as an entry with `secret_key = <old>`,
    ///    `creation = <when the old key first went into service>`,
    ///    `retired = <now>`. Set the top-level `secret_key` to the new value.
    /// 3. Roll the API server. New tokens are signed with the new key;
    ///    tokens minted before the roll keep validating against the entry
    ///    in `previous_secret_keys` provided their `iat` (or, for API
    ///    tokens, the DB row's `creation`) falls in the
    ///    `[creation, retired]` window.
    /// 4. Once the longest token TTL has elapsed (365 days for API keys;
    ///    `u32::MAX` for invites — plan accordingly), remove the entry and
    ///    roll again.
    /// 5. **If a key is suspected compromised**: rotate as above, but set
    ///    `retired` in `previous_secret_keys` to the moment of suspected
    ///    compromise (which may be earlier than the actual demotion). Any
    ///    token whose `creation` falls after that moment will be rejected
    ///    even if validly signed by the compromised key.
    pub fn new_with_previous(
        issuer: String,
        secret_key: &Secret,
        previous: &[JsonPreviousSecretKey],
    ) -> Self {
        let previous = previous
            .iter()
            .map(|entry| PreviousKey {
                decoding: DecodingKey::from_secret(entry.secret_key.as_ref().as_bytes()),
                creation: entry.creation,
                retired: entry.retired,
            })
            .collect();
        Self {
            issuer,
            encoding: EncodingKey::from_secret(secret_key.as_ref().as_bytes()),
            decoding: DecodingKey::from_secret(secret_key.as_ref().as_bytes()),
            previous,
        }
    }

    /// Decode `token` against the current key first, then any previous keys.
    /// Non-signature errors (audience, issuer, expiration, malformed shape)
    /// from the current key short-circuit so we don't fall through to
    /// previous keys for problems that aren't about the key.
    ///
    /// A previous key only matches if both the signature is valid AND the
    /// token's `iat` lies inside that key's `[creation, retired]` window.
    fn decode_with_rotation<C>(
        &self,
        token: &Jwt,
        validation: &Validation,
    ) -> Result<(TokenData<C>, KeyMatch), jsonwebtoken::errors::Error>
    where
        C: DeserializeOwned + HasIat,
    {
        let current_error = match decode::<C>(token.as_ref(), &self.decoding, validation) {
            Ok(data) => return Ok((data, KeyMatch::Current)),
            Err(e) if !matches!(e.kind(), JsonWebTokenErrorKind::InvalidSignature) => {
                return Err(e);
            },
            Err(e) => e,
        };
        self.previous
            .iter()
            .find_map(|prev| {
                let data = decode::<C>(token.as_ref(), &prev.decoding, validation).ok()?;
                let iat = data.claims.iat();
                (iat >= prev.creation.timestamp() && iat <= prev.retired.timestamp()).then_some((
                    data,
                    KeyMatch::Previous {
                        creation: prev.creation,
                        retired: prev.retired,
                    },
                ))
            })
            .ok_or(current_error)
    }

    fn new_jwt(
        &self,
        audience: Audience,
        email: Email,
        ttl: u32,
        org: Option<OrgClaims>,
        state: Option<StateClaims>,
        oci: Option<OciScopeClaims>,
    ) -> Result<Jwt, TokenError> {
        let claims = Claims::new(audience, self.issuer.clone(), email, ttl, org, state, oci);
        Jwt::from_str(
            &encode(&HEADER, &claims, &self.encoding)
                .map_err(|e| TokenError::Encode { error: e })?,
        )
        .map_err(TokenError::Parse)
    }

    pub fn new_auth(&self, email: Email, ttl: u32) -> Result<Jwt, TokenError> {
        self.new_jwt(Audience::Auth, email, ttl, None, None, None)
    }

    pub fn new_client(&self, email: Email, ttl: u32) -> Result<Jwt, TokenError> {
        self.new_jwt(Audience::Client, email, ttl, None, None, None)
    }

    pub fn new_api_key(&self, email: Email, ttl: u32) -> Result<Jwt, TokenError> {
        self.new_jwt(Audience::ApiKey, email, ttl, None, None, None)
    }

    pub fn new_invite(
        &self,
        email: Email,
        ttl: u32,
        org_uuid: OrganizationUuid,
        role: OrganizationRole,
    ) -> Result<Jwt, TokenError> {
        let org_claims = OrgClaims {
            uuid: org_uuid,
            role,
        };
        self.new_jwt(Audience::Invite, email, ttl, Some(org_claims), None, None)
    }

    pub fn new_oauth(&self, email: Email, ttl: u32, state: StateClaims) -> Result<Jwt, TokenError> {
        self.new_jwt(Audience::OAuth, email, ttl, None, Some(state), None)
    }

    pub fn new_oci_public(
        &self,
        ttl: u32,
        repository: Option<String>,
        actions: Vec<OciAction>,
    ) -> Result<Jwt, TokenError> {
        let now = Utc::now().timestamp();
        let claims = PublicOciTokenClaims {
            aud: Audience::OciPublic.into(),
            exp: now.checked_add(i64::from(ttl)).unwrap_or(now),
            iat: now,
            iss: self.issuer.clone(),
            oci: Some(OciScopeClaims {
                repository,
                actions,
            }),
        };
        Jwt::from_str(
            &encode(&HEADER, &claims, &self.encoding)
                .map_err(|e| TokenError::Encode { error: e })?,
        )
        .map_err(TokenError::Parse)
    }

    pub fn new_oci_auth(
        &self,
        email: Email,
        ttl: u32,
        repository: Option<String>,
        actions: Vec<OciAction>,
    ) -> Result<Jwt, TokenError> {
        let oci_claims = OciScopeClaims {
            repository,
            actions,
        };
        self.new_jwt(Audience::OciAuth, email, ttl, None, None, Some(oci_claims))
    }

    #[cfg(feature = "plus")]
    pub fn new_oci_project(
        &self,
        project_uuid: ProjectUuid,
        ttl: u32,
        repository: Option<String>,
        actions: Vec<OciAction>,
    ) -> Result<Jwt, TokenError> {
        let now = Utc::now().timestamp();
        let claims = ProjectOciTokenClaims {
            aud: Audience::OciProject.into(),
            exp: now.checked_add(i64::from(ttl)).unwrap_or(now),
            iat: now,
            iss: self.issuer.clone(),
            sub: project_uuid,
            oci: Some(OciScopeClaims {
                repository,
                actions,
            }),
        };
        Jwt::from_str(
            &encode(&HEADER, &claims, &self.encoding)
                .map_err(|e| TokenError::Encode { error: e })?,
        )
        .map_err(TokenError::Parse)
    }

    #[cfg(feature = "plus")]
    pub fn new_oci_runner(
        &self,
        runner_uuid: RunnerUuid,
        ttl: u32,
        repository: Option<String>,
        actions: Vec<OciAction>,
    ) -> Result<Jwt, TokenError> {
        let now = Utc::now().timestamp();
        let claims = RunnerOciTokenClaims {
            aud: Audience::OciRunner.into(),
            exp: now.checked_add(i64::from(ttl)).unwrap_or(now),
            iat: now,
            iss: self.issuer.clone(),
            sub: runner_uuid,
            oci: Some(OciScopeClaims {
                repository,
                actions,
            }),
        };
        Jwt::from_str(
            &encode(&HEADER, &claims, &self.encoding)
                .map_err(|e| TokenError::Encode { error: e })?,
        )
        .map_err(TokenError::Parse)
    }

    fn validate(
        &self,
        token: &Jwt,
        audience: &[Audience],
    ) -> Result<(TokenData<Claims>, KeyMatch), TokenError> {
        let mut validation = Validation::new(ALGORITHM);
        validation.set_audience(audience);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_required_spec_claims(&["aud", "exp", "iss", "sub"]);

        let (token_data, key_match) = self
            .decode_with_rotation::<Claims>(token, &validation)
            .map_err(|error| TokenError::Decode { error })?;
        let exp = token_data.claims.exp;
        let now = Utc::now().timestamp();
        if exp < now {
            Err(TokenError::Expired {
                exp,
                now,
                error: JsonWebTokenErrorKind::ExpiredSignature.into(),
            })
        } else {
            Ok((token_data, key_match))
        }
    }

    pub fn validate_auth(&self, token: &Jwt) -> Result<Claims, TokenError> {
        Ok(self.validate(token, &[Audience::Auth])?.0.claims)
    }

    pub fn validate_client(&self, token: &Jwt) -> Result<Claims, TokenError> {
        Ok(self
            .validate(token, &[Audience::Client, Audience::ApiKey])?
            .0
            .claims)
    }

    /// Like [`Self::validate_client`] but also surfaces which key validated
    /// the token so callers (i.e. the API-token auth path) can enforce
    /// additional constraints when a previous key matched.
    pub fn validate_client_with_match(
        &self,
        token: &Jwt,
    ) -> Result<(Claims, KeyMatch), TokenError> {
        let (data, key_match) = self.validate(token, &[Audience::Client, Audience::ApiKey])?;
        Ok((data.claims, key_match))
    }

    pub fn validate_api_key(&self, token: &Jwt) -> Result<Claims, TokenError> {
        Ok(self.validate(token, &[Audience::ApiKey])?.0.claims)
    }

    pub fn validate_invite(&self, token: &Jwt) -> Result<InviteClaims, TokenError> {
        self.validate(token, &[Audience::Invite])?
            .0
            .claims
            .try_into()
    }

    pub fn validate_oauth(&self, token: &Jwt) -> Result<OAuthClaims, TokenError> {
        self.validate(token, &[Audience::OAuth])?
            .0
            .claims
            .try_into()
    }

    pub fn validate_oci_public(&self, token: &Jwt) -> Result<PublicOciClaims, TokenError> {
        let mut validation = Validation::new(ALGORITHM);
        validation.set_audience(&[Audience::OciPublic]);
        validation.set_issuer(&[self.issuer.as_str()]);
        // No "sub" required — anonymous tokens have no identity
        validation.set_required_spec_claims(&["aud", "exp", "iss"]);

        let (token_data, _key_match) = self
            .decode_with_rotation::<PublicOciTokenClaims>(token, &validation)
            .map_err(|error| TokenError::Decode { error })?;
        let exp = token_data.claims.exp;
        let now = Utc::now().timestamp();
        if exp < now {
            Err(TokenError::Expired {
                exp,
                now,
                error: JsonWebTokenErrorKind::ExpiredSignature.into(),
            })
        } else {
            token_data.claims.try_into()
        }
    }

    pub fn validate_oci_auth(&self, token: &Jwt) -> Result<AuthOciClaims, TokenError> {
        self.validate(token, &[Audience::OciAuth])?
            .0
            .claims
            .try_into()
    }

    #[cfg(feature = "plus")]
    pub fn validate_oci_project(&self, token: &Jwt) -> Result<ProjectOciClaims, TokenError> {
        let mut validation = Validation::new(ALGORITHM);
        validation.set_audience(&[Audience::OciProject]);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_required_spec_claims(&["aud", "exp", "iss", "sub"]);

        let (token_data, _key_match) = self
            .decode_with_rotation::<ProjectOciTokenClaims>(token, &validation)
            .map_err(|error| TokenError::Decode { error })?;
        let exp = token_data.claims.exp;
        let now = Utc::now().timestamp();
        if exp < now {
            Err(TokenError::Expired {
                exp,
                now,
                error: JsonWebTokenErrorKind::ExpiredSignature.into(),
            })
        } else {
            token_data.claims.try_into()
        }
    }

    #[cfg(feature = "plus")]
    pub fn validate_oci_runner(&self, token: &Jwt) -> Result<RunnerOciClaims, TokenError> {
        let mut validation = Validation::new(ALGORITHM);
        validation.set_audience(&[Audience::OciRunner]);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_required_spec_claims(&["aud", "exp", "iss", "sub"]);

        let (token_data, _key_match) = self
            .decode_with_rotation::<RunnerOciTokenClaims>(token, &validation)
            .map_err(|error| TokenError::Decode { error })?;
        let exp = token_data.claims.exp;
        let now = Utc::now().timestamp();
        if exp < now {
            Err(TokenError::Expired {
                exp,
                now,
                error: JsonWebTokenErrorKind::ExpiredSignature.into(),
            })
        } else {
            token_data.claims.try_into()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr as _, sync::LazyLock};

    use bencher_json::{Email, Jwt, OrganizationUuid, organization::member::OrganizationRole};

    use crate::{Audience, Claims, DEFAULT_SECRET_KEY, OciAction, OciScopeClaims, OrgClaims};

    use super::TokenKey;

    const BENCHER_DOT_DEV_ISSUER: &str = "bencher.dev";
    const TTL: u32 = u32::MAX;

    static EMAIL: LazyLock<Email> = LazyLock::new(|| "info@bencher.dev".parse().unwrap());

    fn make_expired_token(
        secret_key: &TokenKey,
        audience: Audience,
        org: Option<OrgClaims>,
        oci: Option<OciScopeClaims>,
    ) -> Jwt {
        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            aud: audience.to_string(),
            exp: now - 100,
            iat: now - 200,
            iss: BENCHER_DOT_DEV_ISSUER.to_owned(),
            sub: EMAIL.clone(),
            org,
            state: None,
            oci,
        };
        Jwt::from_str(&jsonwebtoken::encode(&super::HEADER, &claims, &secret_key.encoding).unwrap())
            .unwrap()
    }

    #[test]
    fn jwt_auth() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);

        let token = secret_key.new_auth(EMAIL.clone(), TTL).unwrap();

        let claims = secret_key.validate_auth(&token).unwrap();

        assert_eq!(claims.aud, Audience::Auth.to_string());
        assert_eq!(claims.iss, BENCHER_DOT_DEV_ISSUER.to_owned());
        assert_eq!(claims.iat, claims.exp - i64::from(TTL));
        assert_eq!(claims.sub, *EMAIL);
    }

    #[test]
    fn jwt_auth_expired() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let token = make_expired_token(&secret_key, Audience::Auth, None, None);
        secret_key.validate_auth(&token).unwrap_err();
    }

    #[test]
    fn jwt_client() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);

        let token = secret_key.new_client(EMAIL.clone(), TTL).unwrap();

        let claims = secret_key.validate_client(&token).unwrap();

        assert_eq!(claims.aud, Audience::Client.to_string());
        assert_eq!(claims.iss, BENCHER_DOT_DEV_ISSUER.to_owned());
        assert_eq!(claims.iat, claims.exp - i64::from(TTL));
        assert_eq!(claims.sub, *EMAIL);
    }

    #[test]
    fn jwt_client_expired() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let token = make_expired_token(&secret_key, Audience::Client, None, None);
        secret_key.validate_client(&token).unwrap_err();
    }

    #[test]
    fn jwt_api_key() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);

        let token = secret_key.new_api_key(EMAIL.clone(), TTL).unwrap();

        let claims = secret_key.validate_api_key(&token).unwrap();

        assert_eq!(claims.aud, Audience::ApiKey.to_string());
        assert_eq!(claims.iss, BENCHER_DOT_DEV_ISSUER.to_owned());
        assert_eq!(claims.iat, claims.exp - i64::from(TTL));
        assert_eq!(claims.sub, *EMAIL);
    }

    #[test]
    fn jwt_api_key_expired() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let token = make_expired_token(&secret_key, Audience::ApiKey, None, None);
        secret_key.validate_api_key(&token).unwrap_err();
    }

    #[test]
    fn jwt_invite() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);

        let org_uuid = OrganizationUuid::new();
        let role = OrganizationRole::Leader;

        let token = secret_key
            .new_invite(EMAIL.clone(), TTL, org_uuid, role)
            .unwrap();

        let claims = secret_key.validate_invite(&token).unwrap();

        assert_eq!(claims.aud, Audience::Invite.to_string());
        assert_eq!(claims.iss, BENCHER_DOT_DEV_ISSUER.to_owned());
        assert_eq!(claims.iat, claims.exp - i64::from(TTL));
        assert_eq!(claims.sub, *EMAIL);

        assert_eq!(claims.org.uuid, org_uuid);
        assert_eq!(claims.org.role, role);
    }

    #[test]
    fn jwt_invite_expired() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let org = OrgClaims {
            uuid: OrganizationUuid::new(),
            role: OrganizationRole::Leader,
        };
        let token = make_expired_token(&secret_key, Audience::Invite, Some(org), None);
        secret_key.validate_invite(&token).unwrap_err();
    }

    // --- OCI Public tokens ---

    #[test]
    fn jwt_oci_public() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);

        let repository = Some("test-project".to_owned());
        let actions = vec![OciAction::Push];

        let token = secret_key
            .new_oci_public(TTL, repository.clone(), actions.clone())
            .unwrap();

        let claims = secret_key.validate_oci_public(&token).unwrap();

        assert_eq!(claims.oci.repository, repository);
        assert_eq!(claims.oci.actions, actions);
    }

    #[test]
    fn jwt_oci_public_expired() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);

        let now = chrono::Utc::now().timestamp();
        let claims = crate::claims::PublicOciTokenClaims {
            aud: Audience::OciPublic.to_string(),
            exp: now - 100,
            iat: now - 200,
            iss: BENCHER_DOT_DEV_ISSUER.to_owned(),
            oci: Some(OciScopeClaims {
                repository: None,
                actions: vec![],
            }),
        };
        let token = Jwt::from_str(
            &jsonwebtoken::encode(&super::HEADER, &claims, &secret_key.encoding).unwrap(),
        )
        .unwrap();

        secret_key.validate_oci_public(&token).unwrap_err();
    }

    // --- OCI Auth tokens ---

    #[test]
    fn jwt_oci_auth() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);

        let repository = Some("test-org/test-project".to_owned());
        let actions = vec![OciAction::Pull, OciAction::Push];

        let token = secret_key
            .new_oci_auth(EMAIL.clone(), TTL, repository.clone(), actions.clone())
            .unwrap();

        let claims = secret_key.validate_oci_auth(&token).unwrap();

        assert_eq!(claims.aud, Audience::OciAuth.to_string());
        assert_eq!(claims.iss, BENCHER_DOT_DEV_ISSUER.to_owned());
        assert_eq!(claims.iat, claims.exp - i64::from(TTL));
        assert_eq!(claims.sub, *EMAIL);
        assert_eq!(claims.oci.repository, repository);
        assert_eq!(claims.oci.actions, actions);
    }

    #[test]
    fn jwt_oci_auth_expired() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let oci = OciScopeClaims {
            repository: None,
            actions: vec![OciAction::Pull],
        };
        let token = make_expired_token(&secret_key, Audience::OciAuth, None, Some(oci));
        secret_key.validate_oci_auth(&token).unwrap_err();
    }

    #[test]
    fn jwt_oci_auth_empty_actions() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);

        let token = secret_key
            .new_oci_auth(EMAIL.clone(), TTL, None, vec![])
            .unwrap();

        let claims = secret_key.validate_oci_auth(&token).unwrap();
        assert_eq!(claims.aud, Audience::OciAuth.to_string());
        assert_eq!(claims.sub, *EMAIL);
        assert!(claims.oci.actions.is_empty());
        assert!(claims.oci.repository.is_none());
    }

    // --- OCI Project tokens ---

    #[cfg(feature = "plus")]
    #[test]
    fn jwt_oci_project_round_trip() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);

        let project_uuid = bencher_json::ProjectUuid::new();
        let repository = Some("test-org/test-project".to_owned());
        let actions = vec![OciAction::Pull, OciAction::Push];

        let token = secret_key
            .new_oci_project(project_uuid, TTL, repository.clone(), actions.clone())
            .unwrap();

        let claims = secret_key.validate_oci_project(&token).unwrap();

        assert_eq!(claims.aud, Audience::OciProject.to_string());
        assert_eq!(claims.iss, BENCHER_DOT_DEV_ISSUER.to_owned());
        assert_eq!(claims.iat, claims.exp - i64::from(TTL));
        assert_eq!(claims.sub, project_uuid);
        assert_eq!(claims.project_uuid(), project_uuid);
        assert_eq!(claims.oci.repository, repository);
        assert_eq!(claims.oci.actions, actions);
    }

    #[cfg(feature = "plus")]
    #[test]
    fn jwt_oci_project_expired() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);

        let project_uuid = bencher_json::ProjectUuid::new();
        let now = chrono::Utc::now().timestamp();
        let claims = crate::claims::ProjectOciTokenClaims {
            aud: Audience::OciProject.to_string(),
            exp: now - 100,
            iat: now - 200,
            iss: BENCHER_DOT_DEV_ISSUER.to_owned(),
            sub: project_uuid,
            oci: Some(OciScopeClaims {
                repository: None,
                actions: vec![OciAction::Push],
            }),
        };
        let token = Jwt::from_str(
            &jsonwebtoken::encode(&super::HEADER, &claims, &secret_key.encoding).unwrap(),
        )
        .unwrap();

        secret_key.validate_oci_project(&token).unwrap_err();
    }

    // --- OCI Runner tokens ---

    #[cfg(feature = "plus")]
    #[test]
    fn jwt_oci_runner_round_trip() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);

        let runner_uuid = bencher_json::RunnerUuid::new();
        let repository = Some("test-org/test-project".to_owned());
        let actions = vec![OciAction::Pull];

        let token = secret_key
            .new_oci_runner(runner_uuid, TTL, repository.clone(), actions.clone())
            .unwrap();

        let claims = secret_key.validate_oci_runner(&token).unwrap();

        assert_eq!(claims.aud, Audience::OciRunner.to_string());
        assert_eq!(claims.iss, BENCHER_DOT_DEV_ISSUER.to_owned());
        assert_eq!(claims.iat, claims.exp - i64::from(TTL));
        assert_eq!(claims.sub, runner_uuid);
        assert_eq!(claims.runner_uuid(), runner_uuid);
        assert_eq!(claims.oci.repository, repository);
        assert_eq!(claims.oci.actions, actions);
    }

    #[cfg(feature = "plus")]
    #[test]
    fn jwt_oci_runner_expired() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);

        let runner_uuid = bencher_json::RunnerUuid::new();
        let now = chrono::Utc::now().timestamp();
        let claims = crate::claims::RunnerOciTokenClaims {
            aud: Audience::OciRunner.to_string(),
            exp: now - 100,
            iat: now - 200,
            iss: BENCHER_DOT_DEV_ISSUER.to_owned(),
            sub: runner_uuid,
            oci: Some(OciScopeClaims {
                repository: None,
                actions: vec![OciAction::Pull],
            }),
        };
        let token = Jwt::from_str(
            &jsonwebtoken::encode(&super::HEADER, &claims, &secret_key.encoding).unwrap(),
        )
        .unwrap();

        secret_key.validate_oci_runner(&token).unwrap_err();
    }

    // --- Cross-type rejection: each OCI token type rejected by the other ---

    #[test]
    fn jwt_auth_token_rejected_as_oci_auth() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let token = secret_key.new_auth(EMAIL.clone(), TTL).unwrap();
        secret_key.validate_oci_auth(&token).unwrap_err();
    }

    #[test]
    fn jwt_oci_auth_token_rejected_as_auth() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let token = secret_key
            .new_oci_auth(EMAIL.clone(), TTL, None, vec![OciAction::Pull])
            .unwrap();
        secret_key.validate_auth(&token).unwrap_err();
    }

    #[test]
    fn jwt_oci_public_rejected_as_oci_auth() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let token = secret_key.new_oci_public(TTL, None, vec![]).unwrap();
        secret_key.validate_oci_auth(&token).unwrap_err();
    }

    #[test]
    fn jwt_oci_auth_rejected_as_oci_public() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let token = secret_key
            .new_oci_auth(EMAIL.clone(), TTL, None, vec![])
            .unwrap();
        secret_key.validate_oci_public(&token).unwrap_err();
    }

    #[cfg(feature = "plus")]
    #[test]
    fn jwt_oci_public_rejected_as_oci_runner() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let token = secret_key.new_oci_public(TTL, None, vec![]).unwrap();
        secret_key.validate_oci_runner(&token).unwrap_err();
    }

    #[cfg(feature = "plus")]
    #[test]
    fn jwt_oci_runner_rejected_as_oci_public() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let runner_uuid = bencher_json::RunnerUuid::new();
        let token = secret_key
            .new_oci_runner(runner_uuid, TTL, None, vec![OciAction::Pull])
            .unwrap();
        secret_key.validate_oci_public(&token).unwrap_err();
    }

    #[cfg(feature = "plus")]
    #[test]
    fn jwt_oci_auth_rejected_as_oci_runner() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let token = secret_key
            .new_oci_auth(EMAIL.clone(), TTL, None, vec![OciAction::Pull])
            .unwrap();
        secret_key.validate_oci_runner(&token).unwrap_err();
    }

    #[cfg(feature = "plus")]
    #[test]
    fn jwt_oci_runner_rejected_as_oci_auth() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let runner_uuid = bencher_json::RunnerUuid::new();
        let token = secret_key
            .new_oci_runner(runner_uuid, TTL, None, vec![OciAction::Pull])
            .unwrap();
        secret_key.validate_oci_auth(&token).unwrap_err();
    }

    // --- Cross-type rejection: OCI Project tokens ---

    #[cfg(feature = "plus")]
    #[test]
    fn jwt_oci_public_rejected_as_oci_project() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let token = secret_key.new_oci_public(TTL, None, vec![]).unwrap();
        secret_key.validate_oci_project(&token).unwrap_err();
    }

    #[cfg(feature = "plus")]
    #[test]
    fn jwt_oci_project_rejected_as_oci_public() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let project_uuid = bencher_json::ProjectUuid::new();
        let token = secret_key
            .new_oci_project(project_uuid, TTL, None, vec![OciAction::Push])
            .unwrap();
        secret_key.validate_oci_public(&token).unwrap_err();
    }

    #[cfg(feature = "plus")]
    #[test]
    fn jwt_oci_auth_rejected_as_oci_project() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let token = secret_key
            .new_oci_auth(EMAIL.clone(), TTL, None, vec![OciAction::Pull])
            .unwrap();
        secret_key.validate_oci_project(&token).unwrap_err();
    }

    #[cfg(feature = "plus")]
    #[test]
    fn jwt_oci_project_rejected_as_oci_auth() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let project_uuid = bencher_json::ProjectUuid::new();
        let token = secret_key
            .new_oci_project(project_uuid, TTL, None, vec![OciAction::Push])
            .unwrap();
        secret_key.validate_oci_auth(&token).unwrap_err();
    }

    #[cfg(feature = "plus")]
    #[test]
    fn jwt_oci_runner_rejected_as_oci_project() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let runner_uuid = bencher_json::RunnerUuid::new();
        let token = secret_key
            .new_oci_runner(runner_uuid, TTL, None, vec![OciAction::Pull])
            .unwrap();
        secret_key.validate_oci_project(&token).unwrap_err();
    }

    #[cfg(feature = "plus")]
    #[test]
    fn jwt_oci_project_rejected_as_oci_runner() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let project_uuid = bencher_json::ProjectUuid::new();
        let token = secret_key
            .new_oci_project(project_uuid, TTL, None, vec![OciAction::Push])
            .unwrap();
        secret_key.validate_oci_runner(&token).unwrap_err();
    }

    // --- Key rotation ---

    use bencher_json::{DateTime, Secret, system::config::JsonPreviousSecretKey};
    use chrono::Duration;

    use crate::KeyMatch;

    // Fixed window: T0 = TEST - 1h, T1 = TEST + 1h; "now" surrogate = TEST.
    fn old_secret() -> Secret {
        "rotation-old-secret-key".parse().unwrap()
    }
    fn new_secret() -> Secret {
        "rotation-new-secret-key".parse().unwrap()
    }
    fn unknown_secret() -> Secret {
        "rotation-unknown-secret-key".parse().unwrap()
    }
    fn window_start() -> DateTime {
        DateTime::TEST - Duration::seconds(3600)
    }
    fn window_end() -> DateTime {
        DateTime::TEST + Duration::seconds(3600)
    }

    fn previous_entry(secret: Secret) -> JsonPreviousSecretKey {
        JsonPreviousSecretKey {
            secret_key: secret,
            creation: window_start(),
            retired: window_end(),
        }
    }

    /// Mint a JWT with caller-controlled `iat` and `exp`, signed by the given
    /// raw secret. Lets us exercise window boundaries without monkey-patching
    /// the system clock.
    fn mint_with_iat(secret: &Secret, audience: Audience, iat: i64, exp: i64) -> Jwt {
        let claims = Claims {
            aud: audience.to_string(),
            exp,
            iat,
            iss: BENCHER_DOT_DEV_ISSUER.to_owned(),
            sub: EMAIL.clone(),
            org: None,
            state: None,
            oci: None,
        };
        let encoding = jsonwebtoken::EncodingKey::from_secret(secret.as_ref().as_bytes());
        Jwt::from_str(&jsonwebtoken::encode(&super::HEADER, &claims, &encoding).unwrap()).unwrap()
    }

    fn far_future_exp() -> i64 {
        chrono::Utc::now().timestamp() + 86_400
    }

    #[test]
    fn rotation_token_signed_with_previous_key_in_window_validates() {
        let iat = DateTime::TEST.timestamp();
        let exp = far_future_exp();
        let token = mint_with_iat(&old_secret(), Audience::Auth, iat, exp);

        let key = TokenKey::new_with_previous(
            BENCHER_DOT_DEV_ISSUER.to_owned(),
            &new_secret(),
            &[previous_entry(old_secret())],
        );

        let claims = key.validate_auth(&token).unwrap();
        assert_eq!(claims.iat, iat);
    }

    #[test]
    fn rotation_token_signed_with_previous_key_before_window_rejected() {
        let iat = window_start().timestamp() - 1;
        let exp = far_future_exp();
        let token = mint_with_iat(&old_secret(), Audience::Auth, iat, exp);

        let key = TokenKey::new_with_previous(
            BENCHER_DOT_DEV_ISSUER.to_owned(),
            &new_secret(),
            &[previous_entry(old_secret())],
        );

        key.validate_auth(&token).unwrap_err();
    }

    #[test]
    fn rotation_token_signed_with_previous_key_after_window_rejected() {
        let iat = window_end().timestamp() + 1;
        let exp = far_future_exp();
        let token = mint_with_iat(&old_secret(), Audience::Auth, iat, exp);

        let key = TokenKey::new_with_previous(
            BENCHER_DOT_DEV_ISSUER.to_owned(),
            &new_secret(),
            &[previous_entry(old_secret())],
        );

        key.validate_auth(&token).unwrap_err();
    }

    #[test]
    fn rotation_token_signed_with_unknown_key_fails() {
        let iat = DateTime::TEST.timestamp();
        let exp = far_future_exp();
        let token = mint_with_iat(&unknown_secret(), Audience::Auth, iat, exp);

        let key = TokenKey::new_with_previous(
            BENCHER_DOT_DEV_ISSUER.to_owned(),
            &new_secret(),
            &[previous_entry(old_secret())],
        );

        key.validate_auth(&token).unwrap_err();
    }

    #[test]
    fn rotation_expired_token_still_rejected_even_if_signed_with_previous_key() {
        // iat inside the window, but already expired
        let iat = DateTime::TEST.timestamp();
        let exp = chrono::Utc::now().timestamp() - 1;
        let token = mint_with_iat(&old_secret(), Audience::Auth, iat, exp);

        let key = TokenKey::new_with_previous(
            BENCHER_DOT_DEV_ISSUER.to_owned(),
            &new_secret(),
            &[previous_entry(old_secret())],
        );

        // An expired token must be rejected even when its signature is valid
        // against a previous key inside the window.
        key.validate_auth(&token).unwrap_err();
    }

    #[test]
    fn rotation_audience_mismatch_short_circuits() {
        // Mint an OciAuth token with the CURRENT (new) secret; validating as
        // Auth should surface an InvalidAudience error from the current key,
        // NOT fall through to the previous-key path.
        let iat = DateTime::TEST.timestamp();
        let exp = far_future_exp();
        let token = mint_with_iat(&new_secret(), Audience::OciAuth, iat, exp);

        let key = TokenKey::new_with_previous(
            BENCHER_DOT_DEV_ISSUER.to_owned(),
            &new_secret(),
            &[previous_entry(old_secret())],
        );

        let err = key.validate_auth(&token).unwrap_err();
        let crate::TokenError::Decode { error } = &err else {
            panic!("expected Decode/InvalidAudience, got {err:?}");
        };
        assert!(
            matches!(
                error.kind(),
                jsonwebtoken::errors::ErrorKind::InvalidAudience
            ),
            "expected InvalidAudience, got {:?}",
            error.kind()
        );
    }

    #[test]
    fn rotation_new_tokens_sign_with_current_key_only() {
        let key = TokenKey::new_with_previous(
            BENCHER_DOT_DEV_ISSUER.to_owned(),
            &new_secret(),
            &[previous_entry(old_secret())],
        );
        let token = key.new_auth(EMAIL.clone(), TTL).unwrap();

        // A fresh TokenKey holding only the OLD secret must NOT validate it.
        let old_only = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &old_secret());
        old_only.validate_auth(&token).unwrap_err();
    }

    #[test]
    fn rotation_oci_public_signed_with_previous_key_in_window_validates() {
        // OCI Public has a different claims shape; this exercises the typed
        // PublicOciTokenClaims path through `decode_with_rotation`.
        let iat = DateTime::TEST.timestamp();
        let exp = far_future_exp();
        let claims = crate::claims::PublicOciTokenClaims {
            aud: Audience::OciPublic.to_string(),
            exp,
            iat,
            iss: BENCHER_DOT_DEV_ISSUER.to_owned(),
            oci: Some(OciScopeClaims {
                repository: Some("rotated-org/rotated-project".to_owned()),
                actions: vec![OciAction::Pull],
            }),
        };
        let encoding = jsonwebtoken::EncodingKey::from_secret(old_secret().as_ref().as_bytes());
        let token =
            Jwt::from_str(&jsonwebtoken::encode(&super::HEADER, &claims, &encoding).unwrap())
                .unwrap();

        let key = TokenKey::new_with_previous(
            BENCHER_DOT_DEV_ISSUER.to_owned(),
            &new_secret(),
            &[previous_entry(old_secret())],
        );

        let validated = key.validate_oci_public(&token).unwrap();
        assert_eq!(validated.oci.actions, vec![OciAction::Pull]);
    }

    #[test]
    fn rotation_validate_client_with_match_returns_previous_window() {
        let iat = DateTime::TEST.timestamp();
        let exp = far_future_exp();
        let token = mint_with_iat(&old_secret(), Audience::ApiKey, iat, exp);

        let key = TokenKey::new_with_previous(
            BENCHER_DOT_DEV_ISSUER.to_owned(),
            &new_secret(),
            &[previous_entry(old_secret())],
        );

        let (claims, key_match) = key.validate_client_with_match(&token).unwrap();
        assert_eq!(claims.iat, iat);
        match key_match {
            KeyMatch::Previous { creation, retired } => {
                assert_eq!(creation.timestamp(), window_start().timestamp());
                assert_eq!(retired.timestamp(), window_end().timestamp());
            },
            KeyMatch::Current => panic!("expected KeyMatch::Previous, got Current"),
        }
    }

    #[test]
    fn rotation_validate_client_with_match_returns_current_when_no_previous_keys() {
        let key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &new_secret());
        let token = key.new_client(EMAIL.clone(), TTL).unwrap();
        let (_claims, key_match) = key.validate_client_with_match(&token).unwrap();
        assert!(matches!(key_match, KeyMatch::Current));
    }
}
