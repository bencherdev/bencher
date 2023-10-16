/// https://github.com/oxidecomputer/cio/blob/master/dropshot-verify-request/src/bearer.rs
use anyhow::Result;
use async_trait::async_trait;
use dropshot::{
    ApiEndpointBodyContentType, ExtensionMode, ExtractorMetadata, HttpError, RequestContext,
    ServerContext, SharedExtractor,
};

use std::marker::PhantomData;

use crate::http::{internal_error, unauthorized, Headers};

/// A token used for bearer authorization
pub struct BearerToken(Option<String>);

impl BearerToken {
    pub fn inner(&self) -> Option<&String> {
        self.0.as_ref()
    }
}

/// Extracting a bearer token should never fail, it should always return either `Ok(Some(BearerToken))`
/// or `Ok(None)`. `None` will be returned in any of the cases that a valid string can not be extracted.
/// This extractor is not responsible for checking the value of the token.
#[async_trait]
impl SharedExtractor for BearerToken {
    async fn from_request<Context: ServerContext>(
        rqctx: &RequestContext<Context>,
    ) -> Result<BearerToken, HttpError> {
        // We do not care why headers may fail, we only care if we can access them
        let headers = Headers::from_request(rqctx).await.ok();

        // Similarly we only care about the presence of the Authorization header
        let header_value = headers.and_then(|headers| {
            if let Some(header) = headers.0.get("Authorization") {
                // If the value provided is not a readable string we will also throw it out
                header.to_str().map(|s| s.to_string()).ok()
            } else {
                None
            }
        });

        // Finally ensure that the value we found is properly formed
        let contents = header_value.and_then(|value| {
            let parts = value.split_once(' ');

            match parts {
                Some(("Bearer", token)) => Some(token.to_string()),
                _ => None,
            }
        });

        Ok(BearerToken(contents))
    }

    fn metadata(_body_content_type: ApiEndpointBodyContentType) -> ExtractorMetadata {
        ExtractorMetadata {
            extension_mode: ExtensionMode::None,
            parameters: vec![],
        }
    }
}

/// A trait that is implemented by entities that can provide a secret token to test a
/// [BearerToken] against.
#[async_trait]
pub trait BearerProvider {
    async fn token() -> Result<String>;
}

/// A placeholder struct that identifies a Bearer token that has been verified against a
/// secret token provided by `T`. This does not carry the token itself.
pub struct Bearer<T> {
    _provider: PhantomData<T>,
}

/// A placeholder struct that identifies a Bearer token that has been verified against a
/// secret token provided by `T`. Unlike [Bearer], this audit struct can be queried directly
/// to determine if verification succeeded.
pub struct BearerAudit<T> {
    verified: bool,
    _provider: PhantomData<T>,
}

impl<T> BearerAudit<T> {
    /// Returns that status of if this request passed verification
    pub fn verified(&self) -> bool {
        self.verified
    }
}

/// Performs a bearer token check on the given request by checking the request headers against
/// some token provider `T`.  This extractor will fail with an [`INTERNAL_SERVER_ERROR`](http::status::StatusCode::INTERNAL_SERVER_ERROR)
/// if the token provider `T` fails to provide a secret to test against. If the user supplied
/// verification fails, then an [`UNAUTHORIZED`](http::status::StatusCode::UNAUTHORIZED) [`HttpError`](dropshot::HttpError) is returned.
#[async_trait]
impl<T> SharedExtractor for Bearer<T>
where
    T: BearerProvider + Send + Sync,
{
    async fn from_request<Context: ServerContext>(
        rqctx: &RequestContext<Context>,
    ) -> Result<Bearer<T>, HttpError> {
        let audit = BearerAudit::<T>::from_request(rqctx).await?;

        if audit.verified {
            Ok(Bearer {
                _provider: PhantomData,
            })
        } else {
            Err(unauthorized())
        }
    }

    fn metadata(_body_content_type: ApiEndpointBodyContentType) -> ExtractorMetadata {
        ExtractorMetadata {
            extension_mode: ExtensionMode::None,
            parameters: vec![],
        }
    }
}

/// Performs a bearer token check on the given request by checking the request headers against
/// some token provider `T`. This extractor should only fail specifically when the token
/// provider fails to return a secret to test against.
#[async_trait]
impl<T> SharedExtractor for BearerAudit<T>
where
    T: BearerProvider + Send + Sync,
{
    async fn from_request<Context: ServerContext>(
        rqctx: &RequestContext<Context>,
    ) -> Result<BearerAudit<T>, HttpError> {
        let expected_token = T::token().await.map_err(|_| internal_error())?;
        let user_token = BearerToken::from_request(rqctx)
            .await
            .map(|token| token.0)
            .unwrap_or(None);

        let verified = Some(expected_token) == user_token;

        if verified {
            log::info!(
                "Successfully verified request via bearer. req_id: {} uri: {}",
                rqctx.request_id,
                rqctx.request.uri()
            );
        } else {
            log::info!(
                "Failed to verify request via bearer. req_id: {} uri: {}",
                rqctx.request_id,
                rqctx.request.uri()
            );
        }

        Ok(BearerAudit {
            verified,
            _provider: PhantomData,
        })
    }

    fn metadata(_body_content_type: ApiEndpointBodyContentType) -> ExtractorMetadata {
        ExtractorMetadata {
            extension_mode: ExtensionMode::None,
            parameters: vec![],
        }
    }
}
