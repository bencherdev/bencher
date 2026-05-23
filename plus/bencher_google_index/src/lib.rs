use std::{str::FromStr, time::Duration};

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use url::Url;

const GOOGLE_INDEXING_SCOPE: &str = "https://www.googleapis.com/auth/indexing";
const GOOGLE_INDEXING_API_URL: &str = "https://indexing.googleapis.com/v3/urlNotifications:publish";
const TOKEN_LIFETIME_SECS: u64 = 3600;
const TOKEN_REFRESH_BUFFER_SECS: u64 = 60;

pub struct GoogleIndex {
    encoding_key: EncodingKey,
    client_email: String,
    token_uri: String,
    cached_token: RwLock<Option<CachedToken>>,
}

struct CachedToken {
    access_token: String,
    expiry: std::time::Instant,
}

#[derive(Debug, thiserror::Error)]
pub enum GoogleIndexError {
    #[error("Failed to deserialize service account info: {0}")]
    Deserialize(serde_json::Error),
    #[error("Failed to create encoding key: {0}")]
    EncodingKey(jsonwebtoken::errors::Error),
    #[error("Failed to encode JWT: {0}")]
    EncodeJwt(jsonwebtoken::errors::Error),
    #[error("Failed to send token request: {0}")]
    TokenRequest(reqwest::Error),
    #[error("Failed to parse token response: {0}")]
    TokenResponse(reqwest::Error),
    #[error("Token response missing access_token")]
    MissingAccessToken,
    #[error("Failed to serialize indexing request: {0}")]
    BadIndex(serde_json::Error),
    #[error("Failed to send indexing request: {0}")]
    BadIndexRequest(reqwest::Error),
    #[error("Bad indexing response: {0:?}")]
    BadIndexResponse(Box<reqwest::Response>),
}

#[derive(Deserialize)]
struct ServiceAccountInfo {
    private_key: String,
    client_email: String,
    token_uri: String,
}

#[derive(Serialize)]
struct Claims {
    iss: String,
    scope: String,
    aud: String,
    iat: u64,
    exp: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    #[expect(dead_code)]
    expires_in: Option<u64>,
}

impl GoogleIndex {
    pub fn new(
        private_key: String,
        client_email: String,
        token_uri: String,
    ) -> Result<Self, GoogleIndexError> {
        let encoding_key = EncodingKey::from_rsa_pem(private_key.as_bytes())
            .map_err(GoogleIndexError::EncodingKey)?;
        Ok(Self {
            encoding_key,
            client_email,
            token_uri,
            cached_token: RwLock::new(None),
        })
    }

    async fn get_token(&self) -> Result<String, GoogleIndexError> {
        {
            let cached = self.cached_token.read().await;
            if let Some(token) = cached.as_ref() {
                if token.expiry
                    > std::time::Instant::now() + Duration::from_secs(TOKEN_REFRESH_BUFFER_SECS)
                {
                    return Ok(token.access_token.clone());
                }
            }
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let claims = Claims {
            iss: self.client_email.clone(),
            scope: GOOGLE_INDEXING_SCOPE.into(),
            aud: self.token_uri.clone(),
            iat: now,
            exp: now + TOKEN_LIFETIME_SECS,
        };

        let header = Header::new(Algorithm::RS256);
        let jwt = jsonwebtoken::encode(&header, &claims, &self.encoding_key)
            .map_err(GoogleIndexError::EncodeJwt)?;

        let client = reqwest::Client::new();
        let response = client
            .post(&self.token_uri)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", &jwt),
            ])
            .send()
            .await
            .map_err(GoogleIndexError::TokenRequest)?;

        let token_response: TokenResponse = response
            .json()
            .await
            .map_err(GoogleIndexError::TokenResponse)?;

        let access_token = token_response
            .access_token
            .ok_or(GoogleIndexError::MissingAccessToken)?;

        let expiry = std::time::Instant::now() + Duration::from_secs(TOKEN_LIFETIME_SECS);
        let mut cached = self.cached_token.write().await;
        *cached = Some(CachedToken {
            access_token: access_token.clone(),
            expiry,
        });

        Ok(access_token)
    }

    pub async fn url_updated(&self, url: Url) -> Result<(), GoogleIndexError> {
        self.index_url(url, JsonIndexingType::UrlUpdated).await
    }

    pub async fn url_deleted(&self, url: Url) -> Result<(), GoogleIndexError> {
        self.index_url(url, JsonIndexingType::UrlDeleted).await
    }

    async fn index_url(
        &self,
        url: Url,
        index_type: JsonIndexingType,
    ) -> Result<(), GoogleIndexError> {
        let access_token = self.get_token().await?;
        let client = reqwest::Client::new();
        let json_indexing = JsonIndexing {
            url,
            r#type: index_type,
        };
        let json_indexing_str =
            serde_json::to_string(&json_indexing).map_err(GoogleIndexError::BadIndex)?;
        let response = client
            .post(GOOGLE_INDEXING_API_URL)
            .bearer_auth(access_token)
            .body(json_indexing_str)
            .send()
            .await
            .map_err(GoogleIndexError::BadIndexRequest)?;
        // https://developers.google.com/search/apis/indexing-api/v3/core-errors
        response
            .status()
            .is_success()
            .then_some(())
            .ok_or_else(|| GoogleIndexError::BadIndexResponse(Box::new(response)))
    }
}

impl FromStr for GoogleIndex {
    type Err = GoogleIndexError;

    fn from_str(key_data: &str) -> Result<Self, Self::Err> {
        let ServiceAccountInfo {
            private_key,
            client_email,
            token_uri,
        } = serde_json::from_str(key_data).map_err(GoogleIndexError::Deserialize)?;
        Self::new(private_key, client_email, token_uri)
    }
}

// https://developers.google.com/search/apis/indexing-api/v3/using-api
#[derive(Debug, Serialize)]
pub struct JsonIndexing {
    pub url: Url,
    pub r#type: JsonIndexingType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JsonIndexingType {
    UrlUpdated,
    UrlDeleted,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Google Index API"]
    async fn google() {
        let service_key = std::fs::read_to_string("google.json").unwrap();
        let google = GoogleIndex::from_str(&service_key).unwrap();
        let test_url_str = "https://bencher.dev/perf/save-walter-white-3250590663";
        let test_url = Url::parse(test_url_str).unwrap();
        google.url_updated(test_url.clone()).await.unwrap();
        google.url_deleted(test_url.clone()).await.unwrap();
    }
}
