use std::str::FromStr;

use bencher_json::Secret;
use bencher_json::{organization::member::JsonOrganizationRole, Email, Jwt};
use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, Header, TokenData, Validation};
use jsonwebtoken::{DecodingKey, EncodingKey};
use once_cell::sync::Lazy;
use uuid::Uuid;

use crate::ApiError;

mod audience;
mod claims;

use audience::Audience;
use claims::{Claims, OrgClaims};

use self::claims::InviteClaims;

static HEADER: Lazy<Header> = Lazy::new(Header::default);
static ALGORITHM: Lazy<Algorithm> = Lazy::new(Algorithm::default);

pub struct SecretKey {
    pub issuer: String,
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
}

#[derive(Debug, thiserror::Error)]
pub enum JwtError {
    #[error("Failed to decode JSON Web Token: {error}")]
    Decode {
        token: Jwt,
        error: jsonwebtoken::errors::Error,
    },
    #[error("Expired JSON Web Token ({exp} < {now}): {error}")]
    Expired {
        exp: u64,
        now: u64,
        error: jsonwebtoken::errors::Error,
    },
    #[error("Invalid organizational invite: {error}")]
    Invite { error: jsonwebtoken::errors::Error },
}

impl SecretKey {
    pub fn new(issuer: String, secret_key: Secret) -> Self {
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
    ) -> Result<Jwt, ApiError> {
        let claims = Claims::new(audience, self.issuer.clone(), email, ttl, org)?;
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

    fn validate(&self, token: &Jwt, audience: &[Audience]) -> Result<TokenData<Claims>, JwtError> {
        let mut validation = Validation::new(*ALGORITHM);
        validation.set_audience(audience);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_required_spec_claims(&["aud", "exp", "iss", "sub"]);

        let token_data: TokenData<Claims> = decode(token.as_ref(), &self.decoding, &validation)
            .map_err(|error| JwtError::Decode {
                token: token.clone(),
                error,
            })?;
        let exp = token_data.claims.exp;
        let now = u64::try_from(Utc::now().timestamp()).expect("Is it 584942419325 yet?");
        if exp < now {
            Err(JwtError::Expired {
                exp,
                now,
                error: jsonwebtoken::errors::ErrorKind::ExpiredSignature.into(),
            })
        } else {
            Ok(token_data)
        }
    }

    pub fn validate_auth(&self, token: &Jwt) -> Result<Claims, JwtError> {
        Ok(self.validate(token, &[Audience::Auth])?.claims)
    }

    pub fn validate_client(&self, token: &Jwt) -> Result<Claims, JwtError> {
        Ok(self
            .validate(token, &[Audience::Client, Audience::ApiKey])?
            .claims)
    }

    pub fn validate_api_key(&self, token: &Jwt) -> Result<Claims, JwtError> {
        Ok(self.validate(token, &[Audience::ApiKey])?.claims)
    }

    pub fn validate_invite(&self, token: &Jwt) -> Result<InviteClaims, JwtError> {
        self.validate(token, &[Audience::Invite])?.claims.try_into()
    }
}

pub fn now() -> u64 {
    u64::try_from(Utc::now().timestamp()).expect("Today is past 1 Jan 1970.")
}

#[cfg(test)]
mod test {
    use std::{thread, time};

    use bencher_json::{organization::member::JsonOrganizationRole, Email};
    use once_cell::sync::Lazy;
    use uuid::Uuid;

    use crate::{config::DEFAULT_SECRET_KEY, context::secret_key::audience::Audience};

    use super::SecretKey;

    const TTL: u32 = u32::MAX;

    pub static BENCHER_DEV_URL: Lazy<String> = Lazy::new(|| "https://bencher.dev".into());
    static EMAIL: Lazy<Email> = Lazy::new(|| "info@bencher.dev".parse().unwrap());

    fn sleep_for_a_second() {
        let second = time::Duration::from_secs(1);
        thread::sleep(second);
    }

    #[test]
    fn test_jwt_auth() {
        let secret_key = SecretKey::new(BENCHER_DEV_URL.clone(), DEFAULT_SECRET_KEY.clone());

        let token = secret_key.new_auth(EMAIL.clone(), TTL).unwrap();

        let claims = secret_key.validate_auth(&token).unwrap();

        assert_eq!(claims.aud, Audience::Auth.to_string());
        assert_eq!(claims.iss, BENCHER_DEV_URL.to_string());
        assert_eq!(claims.iat, claims.exp - u64::from(TTL));
        assert_eq!(claims.sub, EMAIL.to_string());
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

        let claims = secret_key.validate_client(&token).unwrap();

        assert_eq!(claims.aud, Audience::Client.to_string());
        assert_eq!(claims.iss, BENCHER_DEV_URL.to_string());
        assert_eq!(claims.iat, claims.exp - u64::from(TTL));
        assert_eq!(claims.sub, EMAIL.to_string());
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

        let claims = secret_key.validate_api_key(&token).unwrap();

        assert_eq!(claims.aud, Audience::ApiKey.to_string());
        assert_eq!(claims.iss, BENCHER_DEV_URL.to_string());
        assert_eq!(claims.iat, claims.exp - u64::from(TTL));
        assert_eq!(claims.sub, EMAIL.to_string());
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

        let claims = secret_key.validate_invite(&token).unwrap();

        assert_eq!(claims.aud, Audience::Invite.to_string());
        assert_eq!(claims.iss, BENCHER_DEV_URL.to_string());
        assert_eq!(claims.iat, claims.exp - u64::from(TTL));
        assert_eq!(claims.sub, EMAIL.to_string());

        assert_eq!(claims.org.uuid, org_uuid);
        assert_eq!(claims.org.role, role);
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
