use std::str::FromStr;

use bencher_json::{organization::member::JsonOrganizationRole, Email, Jwt, Secret};
use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, Header, TokenData, Validation};
use jsonwebtoken::{DecodingKey, EncodingKey};
use once_cell::sync::Lazy;
use url::Url;
use uuid::Uuid;

use crate::ApiError;

mod audience;
mod claims;

use audience::Audience;
use claims::{Claims, OrgClaims};

const BENCHER_DOT_DEV: &str = "bencher.dev";

static HEADER: Lazy<Header> = Lazy::new(Header::default);
static ALGORITHM: Lazy<Algorithm> = Lazy::new(Algorithm::default);

pub struct SecretKey {
    endpoint: Url,
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
}

impl SecretKey {
    pub fn new(endpoint: Url, secret_key: Secret) -> Self {
        Self {
            endpoint,
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
    ) -> Result<Jwt, ApiError> {
        let claims = Claims::new(audience, self.endpoint.clone(), email, ttl, org)?;
        Ok(Jwt::from_str(&encode(&HEADER, &claims, &self.encoding)?)?)
    }

    pub fn new_auth(&self, email: Email, ttl: u32) -> Result<Jwt, ApiError> {
        self.new_jwt(Audience::Auth, email, ttl, None)
    }

    pub fn new_client(&self, email: Email, ttl: u32) -> Result<Jwt, ApiError> {
        self.new_jwt(Audience::Client, email, ttl, None)
    }

    pub fn new_api_key(&self, email: Email, ttl: u32) -> Result<Jwt, ApiError> {
        self.new_jwt(Audience::ApiKey, email, ttl, None)
    }

    pub fn new_invite(
        &self,
        email: Email,
        ttl: u32,
        org_uuid: Uuid,
        role: JsonOrganizationRole,
    ) -> Result<Jwt, ApiError> {
        let org_claims = OrgClaims {
            uuid: org_uuid,
            role,
        };
        self.new_jwt(Audience::Invite, email, ttl, Some(org_claims))
    }

    fn validate(&self, token: &Jwt, audience: &[Audience]) -> Result<TokenData<Claims>, ApiError> {
        let mut validation = Validation::new(*ALGORITHM);
        validation.set_audience(audience);
        validation.set_issuer(&[BENCHER_DOT_DEV, self.endpoint.as_ref()]);
        validation.set_required_spec_claims(&["aud", "exp", "iss", "sub"]);

        let token_data: TokenData<Claims> = decode(token.as_ref(), &self.decoding, &validation)?;
        check_expiration(token_data.claims.exp)?;

        Ok(token_data)
    }

    pub fn validate_auth(&self, token: &Jwt) -> Result<TokenData<Claims>, ApiError> {
        self.validate(token, &[Audience::Auth])
    }

    pub fn validate_client(&self, token: &Jwt) -> Result<TokenData<Claims>, ApiError> {
        self.validate(token, &[Audience::Client, Audience::ApiKey])
    }

    pub fn validate_api_key(&self, token: &Jwt) -> Result<TokenData<Claims>, ApiError> {
        self.validate(token, &[Audience::ApiKey])
    }

    pub fn validate_invite(&self, token: &Jwt) -> Result<TokenData<Claims>, ApiError> {
        self.validate(token, &[Audience::Invite])
    }
}

fn check_expiration(time: u64) -> Result<(), ApiError> {
    let now = now()?;
    if time < now {
        Err(
            jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::ExpiredSignature)
                .into(),
        )
    } else {
        Ok(())
    }
}

pub fn now() -> Result<u64, ApiError> {
    u64::try_from(Utc::now().timestamp()).map_err(Into::into)
}

#[cfg(test)]
mod test {
    use std::{thread, time};

    use bencher_json::{organization::member::JsonOrganizationRole, Email};
    use once_cell::sync::Lazy;
    use url::Url;
    use uuid::Uuid;

    use crate::{config::DEFAULT_SECRET_KEY, context::secret_key::audience::Audience};

    use super::SecretKey;

    const TTL: u32 = u32::MAX;

    pub static BENCHER_DEV_URL: Lazy<Url> = Lazy::new(|| "https://bencher.dev".parse().unwrap());
    static EMAIL: Lazy<Email> = Lazy::new(|| "info@bencher.dev".parse().unwrap());

    fn sleep_for_a_second() {
        let second = time::Duration::from_secs(1);
        thread::sleep(second);
    }

    #[test]
    fn test_jwt_auth() {
        let secret_key = SecretKey::new(BENCHER_DEV_URL.clone(), DEFAULT_SECRET_KEY.clone());

        let token = secret_key.new_auth(EMAIL.clone(), TTL).unwrap();

        let token_data = secret_key.validate_auth(&token).unwrap();

        assert_eq!(token_data.claims.aud, Audience::Auth.to_string());
        assert_eq!(token_data.claims.iss, BENCHER_DEV_URL.to_string());
        assert_eq!(
            token_data.claims.iat,
            token_data.claims.exp - u64::from(TTL)
        );
        assert_eq!(token_data.claims.sub, EMAIL.to_string());
    }

    #[test]
    fn test_jwt_auth_expired() {
        let secret_key = SecretKey::new(BENCHER_DEV_URL.clone(), DEFAULT_SECRET_KEY.clone());

        let token = secret_key.new_auth(EMAIL.clone(), 0).unwrap();

        sleep_for_a_second();

        assert!(secret_key.validate_auth(&token).is_err());
    }

    #[test]
    fn test_jwt_client() {
        let secret_key = SecretKey::new(BENCHER_DEV_URL.clone(), DEFAULT_SECRET_KEY.clone());

        let token = secret_key.new_client(EMAIL.clone(), TTL).unwrap();

        let token_data = secret_key.validate_client(&token).unwrap();

        assert_eq!(token_data.claims.aud, Audience::Client.to_string());
        assert_eq!(token_data.claims.iss, BENCHER_DEV_URL.to_string());
        assert_eq!(
            token_data.claims.iat,
            token_data.claims.exp - u64::from(TTL)
        );
        assert_eq!(token_data.claims.sub, EMAIL.to_string());
    }

    #[test]
    fn test_jwt_client_expired() {
        let secret_key = SecretKey::new(BENCHER_DEV_URL.clone(), DEFAULT_SECRET_KEY.clone());

        let token = secret_key.new_client(EMAIL.clone(), 0).unwrap();

        sleep_for_a_second();

        assert!(secret_key.validate_client(&token).is_err());
    }

    #[test]
    fn test_jwt_api_key() {
        let secret_key = SecretKey::new(BENCHER_DEV_URL.clone(), DEFAULT_SECRET_KEY.clone());

        let token = secret_key.new_api_key(EMAIL.clone(), TTL).unwrap();

        let token_data = secret_key.validate_api_key(&token).unwrap();

        assert_eq!(token_data.claims.aud, Audience::ApiKey.to_string());
        assert_eq!(token_data.claims.iss, BENCHER_DEV_URL.to_string());
        assert_eq!(
            token_data.claims.iat,
            token_data.claims.exp - u64::from(TTL)
        );
        assert_eq!(token_data.claims.sub, EMAIL.to_string());
    }

    #[test]
    fn test_jwt_api_key_expired() {
        let secret_key = SecretKey::new(BENCHER_DEV_URL.clone(), DEFAULT_SECRET_KEY.clone());

        let token = secret_key.new_api_key(EMAIL.clone(), 0).unwrap();

        sleep_for_a_second();

        assert!(secret_key.validate_api_key(&token).is_err());
    }

    #[test]
    fn test_jwt_invite() {
        let secret_key = SecretKey::new(BENCHER_DEV_URL.clone(), DEFAULT_SECRET_KEY.clone());

        let org_uuid = Uuid::new_v4();
        let role = JsonOrganizationRole::Leader;

        let token = secret_key
            .new_invite(EMAIL.clone(), TTL, org_uuid, role)
            .unwrap();

        let token_data = secret_key.validate_invite(&token).unwrap();

        assert_eq!(token_data.claims.aud, Audience::Invite.to_string());
        assert_eq!(token_data.claims.iss, BENCHER_DEV_URL.to_string());
        assert_eq!(
            token_data.claims.iat,
            token_data.claims.exp - u64::from(TTL)
        );
        assert_eq!(token_data.claims.sub, EMAIL.to_string());

        let org_claims = token_data.claims.org.unwrap();
        assert_eq!(org_claims.uuid, org_uuid);
        assert_eq!(org_claims.role, role);
    }

    #[test]
    fn test_jwt_invite_expired() {
        let secret_key = SecretKey::new(BENCHER_DEV_URL.clone(), DEFAULT_SECRET_KEY.clone());

        let org_uuid = Uuid::new_v4();
        let role = JsonOrganizationRole::Leader;

        let token = secret_key
            .new_invite(EMAIL.clone(), 0, org_uuid, role)
            .unwrap();

        sleep_for_a_second();

        assert!(secret_key.validate_invite(&token).is_err());
    }
}
