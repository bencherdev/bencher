use std::str::FromStr as _;
use std::sync::LazyLock;

use bencher_json::{Email, Jwt, OrganizationUuid, Secret, organization::member::OrganizationRole};
use chrono::Utc;
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode,
    errors::ErrorKind as JsonWebTokenErrorKind,
};

use crate::{
    Audience, Claims, InviteClaims, OAuthClaims, OciClaims, OciScopeClaims, OrgClaims, StateClaims,
    TokenError,
};

static HEADER: LazyLock<Header> = LazyLock::new(Header::default);
static ALGORITHM: LazyLock<Algorithm> = LazyLock::new(Algorithm::default);

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
        state: Option<StateClaims>,
        oci: Option<OciScopeClaims>,
    ) -> Result<Jwt, TokenError> {
        let claims = Claims::new(audience, self.issuer.clone(), email, ttl, org, state, oci);
        Jwt::from_str(&encode(&HEADER, &claims, &self.encoding).map_err(|e| {
            TokenError::Encode {
                claims: Box::new(claims),
                error: e,
            }
        })?)
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

    /// Create a new OCI registry access token
    ///
    /// These tokens are short-lived (5 minutes) and contain the authorized
    /// repository and actions in the claims.
    pub fn new_oci(
        &self,
        email: Email,
        ttl: u32,
        repository: Option<String>,
        actions: Vec<String>,
    ) -> Result<Jwt, TokenError> {
        let oci_claims = OciScopeClaims {
            repository,
            actions,
        };
        self.new_jwt(Audience::Oci, email, ttl, None, None, Some(oci_claims))
    }

    fn validate(
        &self,
        token: &Jwt,
        audience: &[Audience],
    ) -> Result<TokenData<Claims>, TokenError> {
        let mut validation = Validation::new(*ALGORITHM);
        validation.set_audience(audience);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_required_spec_claims(&["aud", "exp", "iss", "sub"]);

        let token_data: TokenData<Claims> = decode(token.as_ref(), &self.decoding, &validation)
            .map_err(|error| TokenError::Decode {
                token: token.clone(),
                error,
            })?;
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

    pub fn validate_oci(&self, token: &Jwt) -> Result<OciClaims, TokenError> {
        self.validate(token, &[Audience::Oci])?.claims.try_into()
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::LazyLock, thread, time};

    use bencher_json::{Email, OrganizationUuid, organization::member::OrganizationRole};

    use crate::{Audience, DEFAULT_SECRET_KEY};

    use super::TokenKey;

    const BENCHER_DOT_DEV_ISSUER: &str = "bencher.dev";
    const TTL: u32 = u32::MAX;

    static EMAIL: LazyLock<Email> = LazyLock::new(|| "info@bencher.dev".parse().unwrap());

    fn sleep_for_a_second() {
        let second = time::Duration::from_secs(1);
        thread::sleep(second);
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

        let token = secret_key.new_auth(EMAIL.clone(), 0).unwrap();

        sleep_for_a_second();

        assert!(secret_key.validate_auth(&token).is_err());
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

        let token = secret_key.new_client(EMAIL.clone(), 0).unwrap();

        sleep_for_a_second();

        assert!(secret_key.validate_client(&token).is_err());
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

        let token = secret_key.new_api_key(EMAIL.clone(), 0).unwrap();

        sleep_for_a_second();

        assert!(secret_key.validate_api_key(&token).is_err());
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

        let org_uuid = OrganizationUuid::new();
        let role = OrganizationRole::Leader;

        let token = secret_key
            .new_invite(EMAIL.clone(), 0, org_uuid, role)
            .unwrap();

        sleep_for_a_second();

        assert!(secret_key.validate_invite(&token).is_err());
    }
}
