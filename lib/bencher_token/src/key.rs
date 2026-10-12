use std::str::FromStr as _;
use std::sync::LazyLock;

use bencher_json::{
    DateTime, Email, Jwt, OrganizationUuid, Secret, UserUuid,
    organization::member::OrganizationRole,
};
use chrono::Utc;
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode,
    errors::ErrorKind as JsonWebTokenErrorKind,
};
use uuid::Uuid;

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
use crate::claims::PublicOciTokenClaims;
#[cfg(feature = "plus")]
use crate::claims::RunnerOciTokenClaims;
use crate::{
    Audience, AuthOciClaims, Claims, EmailChangeClaims, InviteClaims, OAuthClaims, OciAction,
    OciScopeClaims, OrgClaims, PublicOciClaims, StateClaims, TokenError,
};

static HEADER: LazyLock<Header> = LazyLock::new(Header::default);
const ALGORITHM: Algorithm = Algorithm::HS256;

pub struct TokenKey {
    pub issuer: String,
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
}

impl TokenKey {
    pub fn new(issuer: String, secret_key: &Secret) -> Self {
        Self {
            issuer,
            encoding: EncodingKey::from_secret(secret_key.as_ref().as_bytes()),
            decoding: DecodingKey::from_secret(secret_key.as_ref().as_bytes()),
        }
    }

    fn new_jwt(
        &self,
        audience: Audience,
        email: Email,
        ttl: u32,
        org: Option<OrgClaims>,
        oci: Option<OciScopeClaims>,
    ) -> Result<Jwt, TokenError> {
        let claims = Claims::new(audience, self.issuer.clone(), email, ttl, org, None, oci);
        Jwt::from_str(
            &encode(&HEADER, &claims, &self.encoding)
                .map_err(|e| TokenError::Encode { error: e })?,
        )
        .map_err(TokenError::Parse)
    }

    pub fn new_auth(&self, email: Email, ttl: u32) -> Result<Jwt, TokenError> {
        self.new_jwt(Audience::Auth, email, ttl, None, None)
    }

    pub fn new_client(&self, email: Email, ttl: u32) -> Result<Jwt, TokenError> {
        self.new_jwt(Audience::Client, email, ttl, None, None)
    }

    pub fn new_api_key(&self, email: Email, ttl: u32) -> Result<Jwt, TokenError> {
        self.new_jwt(Audience::ApiKey, email, ttl, None, None)
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
        self.new_jwt(Audience::Invite, email, ttl, Some(org_claims), None)
    }

    pub fn new_oauth(&self, email: Email, ttl: u32, state: StateClaims) -> Result<Jwt, TokenError> {
        let claims = Claims {
            jti: Some(Uuid::new_v4()),
            ..Claims::new(
                Audience::OAuth,
                self.issuer.clone(),
                email,
                ttl,
                None,
                Some(state),
                None,
            )
        };
        Jwt::from_str(
            &encode(&HEADER, &claims, &self.encoding)
                .map_err(|e| TokenError::Encode { error: e })?,
        )
        .map_err(TokenError::Parse)
    }

    pub fn new_email_change(
        &self,
        now: DateTime,
        ttl: u32,
        user: UserUuid,
        modified: DateTime,
        email: Email,
        new_email: Email,
    ) -> Result<Jwt, TokenError> {
        let now = now.timestamp();
        let claims = EmailChangeClaims {
            aud: Audience::EmailChange.into(),
            exp: now.checked_add(i64::from(ttl)).unwrap_or(now),
            iat: now,
            iss: self.issuer.clone(),
            sub: user,
            modified,
            email,
            new_email,
        };
        Jwt::from_str(
            &encode(&HEADER, &claims, &self.encoding)
                .map_err(|e| TokenError::Encode { error: e })?,
        )
        .map_err(TokenError::Parse)
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
        self.new_jwt(Audience::OciAuth, email, ttl, None, Some(oci_claims))
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
    ) -> Result<TokenData<Claims>, TokenError> {
        let mut validation = Validation::new(ALGORITHM);
        validation.set_audience(audience);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_required_spec_claims(&["aud", "exp", "iss", "sub"]);

        let token_data: TokenData<Claims> = decode(token.as_ref(), &self.decoding, &validation)
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
            Ok(token_data)
        }
    }

    pub fn validate_auth(&self, token: &Jwt) -> Result<Claims, TokenError> {
        Ok(self.validate(token, &[Audience::Auth])?.claims)
    }

    pub fn validate_client(&self, token: &Jwt) -> Result<Claims, TokenError> {
        Ok(self
            .validate(token, &[Audience::Client, Audience::ApiKey])?
            .claims)
    }

    pub fn validate_api_key(&self, token: &Jwt) -> Result<Claims, TokenError> {
        Ok(self.validate(token, &[Audience::ApiKey])?.claims)
    }

    pub fn validate_invite(&self, token: &Jwt) -> Result<InviteClaims, TokenError> {
        self.validate(token, &[Audience::Invite])?.claims.try_into()
    }

    pub fn validate_oauth(&self, token: &Jwt) -> Result<OAuthClaims, TokenError> {
        self.validate(token, &[Audience::OAuth])?.claims.try_into()
    }

    pub fn validate_email_change(
        &self,
        token: &Jwt,
        now: DateTime,
    ) -> Result<EmailChangeClaims, TokenError> {
        let mut validation = Validation::new(ALGORITHM);
        validation.set_audience(&[Audience::EmailChange]);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_required_spec_claims(&["aud", "exp", "iss", "sub"]);
        // Expiry is checked against `now` below, so the server's clock decides it.
        validation.validate_exp = false;

        let token_data: TokenData<EmailChangeClaims> =
            decode(token.as_ref(), &self.decoding, &validation)
                .map_err(|error| TokenError::Decode { error })?;
        let exp = token_data.claims.exp;
        let now = now.timestamp();
        if exp < now {
            Err(TokenError::Expired {
                exp,
                now,
                error: JsonWebTokenErrorKind::ExpiredSignature.into(),
            })
        } else {
            Ok(token_data.claims)
        }
    }

    pub fn validate_oci_public(&self, token: &Jwt) -> Result<PublicOciClaims, TokenError> {
        let mut validation = Validation::new(ALGORITHM);
        validation.set_audience(&[Audience::OciPublic]);
        validation.set_issuer(&[self.issuer.as_str()]);
        // No "sub" required — anonymous tokens have no identity
        validation.set_required_spec_claims(&["aud", "exp", "iss"]);

        let token_data: TokenData<PublicOciTokenClaims> =
            decode(token.as_ref(), &self.decoding, &validation)
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
            .claims
            .try_into()
    }

    #[cfg(feature = "plus")]
    pub fn validate_oci_project(&self, token: &Jwt) -> Result<ProjectOciClaims, TokenError> {
        let mut validation = Validation::new(ALGORITHM);
        validation.set_audience(&[Audience::OciProject]);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_required_spec_claims(&["aud", "exp", "iss", "sub"]);

        let token_data: TokenData<ProjectOciTokenClaims> =
            decode(token.as_ref(), &self.decoding, &validation)
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

        let token_data: TokenData<RunnerOciTokenClaims> =
            decode(token.as_ref(), &self.decoding, &validation)
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

    use bencher_json::{
        DateTime, Email, Jwt, OrganizationUuid, UserUuid, organization::member::OrganizationRole,
    };

    use crate::{
        Audience, Claims, DEFAULT_SECRET_KEY, EmailChangeClaims, OciAction, OciScopeClaims,
        OrgClaims, StateClaims,
    };

    use super::TokenKey;

    const BENCHER_DOT_DEV_ISSUER: &str = "bencher.dev";
    const TTL: u32 = u32::MAX;

    static EMAIL: LazyLock<Email> = LazyLock::new(|| "info@bencher.dev".parse().unwrap());
    static NEW_EMAIL: LazyLock<Email> = LazyLock::new(|| "new@bencher.dev".parse().unwrap());

    /// Long past, so only the `now` a validation is given can accept a token issued then.
    const EMAIL_CHANGE_NOW: i64 = 1_789_344_000;
    const EMAIL_CHANGE_MODIFIED: i64 = EMAIL_CHANGE_NOW - 1;
    const EMAIL_CHANGE_TTL: u32 = 30 * 60;

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
            jti: None,
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

    // Kills a state without its random claim, or with a constant one: two states for the same
    // sign in each carry a JWT ID, and the two differ.
    #[test]
    fn jwt_oauth_states_differ() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let jti = || {
            let state = StateClaims {
                invite: None,
                claim: None,
                #[cfg(feature = "plus")]
                plan: None,
                #[cfg(feature = "plus")]
                authorization: None,
            };
            let token = secret_key.new_oauth(EMAIL.clone(), TTL, state).unwrap();
            secret_key
                .validate(&token, &[Audience::OAuth])
                .unwrap()
                .claims
                .jti
                .expect("No JWT ID")
        };
        assert_ne!(jti(), jti());
    }

    // --- Email change tokens ---

    // Fails if an email change token loses the user, the user's `modified`, or either email,
    // or expires by the system clock.
    #[test]
    fn jwt_email_change() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let user = UserUuid::new();

        let token = email_change_token(&secret_key, user);

        let claims = secret_key
            .validate_email_change(&token, at(EMAIL_CHANGE_NOW))
            .unwrap();
        assert_eq!(claims.aud, Audience::EmailChange.to_string());
        assert_eq!(claims.iss, BENCHER_DOT_DEV_ISSUER.to_owned());
        assert_eq!(claims.sub, user);
        assert_eq!(claims.modified, at(EMAIL_CHANGE_MODIFIED));
        assert_eq!(claims.email, *EMAIL);
        assert_eq!(claims.new_email, *NEW_EMAIL);
    }

    // Fails if an email change token expires by the system clock instead of the given time.
    #[test]
    fn jwt_email_change_expired() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let token = email_change_token(&secret_key, UserUuid::new());
        let expiration = EMAIL_CHANGE_NOW + i64::from(EMAIL_CHANGE_TTL);

        secret_key
            .validate_email_change(&token, at(expiration))
            .unwrap();
        secret_key
            .validate_email_change(&token, at(expiration + 1))
            .unwrap_err();
    }

    // Fails if an email change token is accepted without its own audience.
    #[test]
    fn jwt_email_change_other_audience() {
        let secret_key = TokenKey::new(BENCHER_DOT_DEV_ISSUER.to_owned(), &DEFAULT_SECRET_KEY);
        let claims = EmailChangeClaims {
            aud: Audience::Auth.to_string(),
            exp: EMAIL_CHANGE_NOW + i64::from(EMAIL_CHANGE_TTL),
            iat: EMAIL_CHANGE_NOW,
            iss: BENCHER_DOT_DEV_ISSUER.to_owned(),
            sub: UserUuid::new(),
            modified: at(EMAIL_CHANGE_MODIFIED),
            email: EMAIL.clone(),
            new_email: NEW_EMAIL.clone(),
        };
        let token = Jwt::from_str(
            &jsonwebtoken::encode(&super::HEADER, &claims, &secret_key.encoding).unwrap(),
        )
        .unwrap();

        secret_key
            .validate_email_change(&token, at(EMAIL_CHANGE_NOW))
            .unwrap_err();
    }

    fn email_change_token(secret_key: &TokenKey, user: UserUuid) -> Jwt {
        secret_key
            .new_email_change(
                at(EMAIL_CHANGE_NOW),
                EMAIL_CHANGE_TTL,
                user,
                at(EMAIL_CHANGE_MODIFIED),
                EMAIL.clone(),
                NEW_EMAIL.clone(),
            )
            .unwrap()
    }

    fn at(timestamp: i64) -> DateTime {
        DateTime::try_from(timestamp).unwrap()
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
        assert_eq!(claims.oci.actions, Vec::new());
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
}
