use std::str::FromStr;

use serde::Serialize;
use tame_oauth::{
    gcp::{
        service_account::ServiceAccountProviderInner, ServiceAccountInfo, ServiceAccountProvider,
        TokenOrRequest, TokenProvider,
    },
    token_cache::CachedTokenProvider,
    Token,
};
use url::Url;

const GOOGLE_INDEXING_SCOPE: &str = "https://www.googleapis.com/auth/indexing";
const GOOGLE_INDEXING_API_URL: &str = "https://indexing.googleapis.com/v3/urlNotifications:publish";

pub struct Google {
    inner: CachedTokenProvider<ServiceAccountProviderInner>,
}

#[derive(Debug, thiserror::Error)]
pub enum GoogleError {
    #[error("Failed to deserialize service account info: {0}")]
    Deserialize(tame_oauth::Error),
    #[error("Failed to create service account provider: {0}")]
    CreateProvider(tame_oauth::Error),
    #[error("Failed to create token: {0}")]
    CreateToken(tame_oauth::Error),
    #[error("Unexpected HTTP method, other than POST: {0}")]
    BadMethod(http::Method),
    #[error("Failed to build token request: {0}")]
    BuildRequest(reqwest::Error),
    #[error("Failed to send token request: {0}")]
    BadRequest(reqwest::Error),
    #[error("Failed to create token response headers")]
    BadResponseHeaders,
    #[error("Failed to create token response bytes: {0}")]
    BadResponseBytes(reqwest::Error),
    #[error("Failed to create token response body: {0}")]
    BadResponseBody(http::Error),
    #[error("Failed to parse token response: {0}")]
    ParseToken(tame_oauth::Error),
    #[error("Failed to parse indexing request: {0}")]
    BadIndex(serde_json::Error),
    #[error("Failed to send indexing request: {0}")]
    BadIndexRequest(reqwest::Error),
    #[error("Bad indexing response: {0:?}")]
    BadIndexResponse(reqwest::Response),
}

impl Google {
    pub fn new(
        private_key: String,
        client_email: String,
        token_uri: String,
    ) -> Result<Self, GoogleError> {
        let info = ServiceAccountInfo {
            private_key,
            client_email,
            token_uri,
        };
        Ok(Self {
            inner: ServiceAccountProvider::new(info).map_err(GoogleError::CreateProvider)?,
        })
    }

    pub async fn get_token(&self, scopes: &[&str]) -> Result<Token, GoogleError> {
        match self.inner.get_token(scopes) {
            // Attempt to get a token, since we have never used this accessor
            // before, it's guaranteed that we will need to make an HTTPS
            // request to the token provider to retrieve a token. This
            // will also happen if we want to get a token for a different set
            // of scopes, or if the token has expired.
            Ok(TokenOrRequest::Request {
                // This is an http::Request that we can use to build
                // a client request for whichever HTTP client implementation
                // you wish to use
                request,
                scope_hash,
                ..
            }) => {
                let client = reqwest::Client::new();

                let (parts, body) = request.into_parts();
                let uri = parts.uri.to_string();

                // This will always be a POST, but for completeness sake...
                let builder = match parts.method {
                    http::Method::POST => client.post(&uri),
                    method => return Err(GoogleError::BadMethod(method)),
                };

                // Build the full request from the headers and body that were
                // passed to you, without modifying them.
                let request = builder
                    .headers(parts.headers)
                    .body(body)
                    .build()
                    .map_err(GoogleError::BuildRequest)?;

                // Send the actual request
                let response = client
                    .execute(request)
                    .await
                    .map_err(GoogleError::BadRequest)?;

                let mut builder = http::Response::builder()
                    .status(response.status())
                    .version(response.version());

                let headers = builder
                    .headers_mut()
                    .ok_or(GoogleError::BadResponseHeaders)?;

                // Unfortunately http doesn't expose a way to just use
                // an existing HeaderMap, so we have to copy them :(
                headers.extend(
                    response
                        .headers()
                        .into_iter()
                        .map(|(k, v)| (k.clone(), v.clone())),
                );

                let buffer = response
                    .bytes()
                    .await
                    .map_err(GoogleError::BadResponseBytes)?;
                let response = builder.body(buffer).map_err(GoogleError::BadResponseBody)?;

                // Tell our accessor about the response, also passing
                // the scope_hash for the scopes we initially requested,
                // this will allow future token requests for those scopes
                // to use a cached token, at least until it expires (~1 hour)
                self.inner
                    .parse_token_response(scope_hash, response)
                    .map_err(GoogleError::ParseToken)
            },
            // Retrieving a token for the same scopes for which a token has been acquired
            // will use the cached token until it expires
            Ok(TokenOrRequest::Token(token)) => Ok(token),
            Err(e) => Err(GoogleError::CreateToken(e)),
        }
    }

    pub async fn url_updated(&self, url: Url) -> Result<(), GoogleError> {
        self.index_url(url, JsonIndexingType::UrlUpdated).await
    }

    pub async fn url_deleted(&self, url: Url) -> Result<(), GoogleError> {
        self.index_url(url, JsonIndexingType::UrlDeleted).await
    }

    async fn index_url(&self, url: Url, index_type: JsonIndexingType) -> Result<(), GoogleError> {
        let token = self.get_token(&[GOOGLE_INDEXING_SCOPE]).await?;
        let client = reqwest::Client::new();
        let json_indexing = JsonIndexing {
            url,
            r#type: index_type,
        };
        let json_indexing_str =
            serde_json::to_string(&json_indexing).map_err(GoogleError::BadIndex)?;
        let response = client
            .post(GOOGLE_INDEXING_API_URL)
            .bearer_auth(token.access_token)
            .body(json_indexing_str)
            .send()
            .await
            .map_err(GoogleError::BadIndexRequest)?;
        // https://developers.google.com/search/apis/indexing-api/v3/core-errors
        response
            .status()
            .is_success()
            .then_some(())
            .ok_or_else(|| GoogleError::BadIndexResponse(response))
    }
}

impl FromStr for Google {
    type Err = GoogleError;

    fn from_str(key_data: &str) -> Result<Self, Self::Err> {
        // https://github.com/EmbarkStudios/tame-oauth/blob/main/examples/svc_account.rs
        let ServiceAccountInfo {
            private_key,
            client_email,
            token_uri,
        } = ServiceAccountInfo::deserialize(key_data).map_err(GoogleError::Deserialize)?;
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_google() {
        let service_key = std::fs::read_to_string("google.json").unwrap();
        let google = Google::from_str(&service_key).unwrap();
        let test_url_str = "https://bencher.dev/perf/save-walter-white-3250590663";
        let test_url = Url::parse(test_url_str).unwrap();
        google.url_updated(test_url.clone()).await.unwrap();
        google.url_deleted(test_url.clone()).await.unwrap();
    }
}
