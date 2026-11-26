//! Protocol error bodies for the OCI registry and its token endpoint
//!
//! dropshot sends an `HttpError` inside its own envelope, `{"request_id", "message"}`,
//! and it leaves the headers off an endpoint's custom error type.
//! So these endpoints build each error response themselves, with their protocol's
//! error body at the top level and every header the error carries.
//! `ProtocolError` is their dropshot error type, for the errors raised before they run.

use std::fmt;

use bencher_json::oci::{
    OCI_ERROR_BLOB_UPLOAD_INVALID, OCI_ERROR_CODES, OCI_ERROR_DENIED, OCI_ERROR_NAME_UNKNOWN,
    OCI_ERROR_SIZE_INVALID, OCI_ERROR_TOO_MANY_REQUESTS, OCI_ERROR_UNAUTHORIZED, OCI_ERROR_UNKNOWN,
    OCI_ERROR_UNSUPPORTED,
};
use dropshot::{Body, ErrorStatusCode, HttpError, HttpResponseError};
use http::{HeaderMap, HeaderValue, Response, StatusCode};
use schemars::JsonSchema;
use serde::Serialize;
use slog::Logger;

/// An error with the OCI error body, `{"errors": [{"code", "message"}]}`
pub type RegistryError = ProtocolError<OciErrors>;

/// An `OAuth2` token endpoint error with its RFC 6749 §5.2 body, `{"error", "error_description"}`
pub type OAuthError = ProtocolError<OAuthErrorBody>;

/// The endpoint's response, or its error as a response with the protocol's error body.
pub fn protocol_response<B>(
    log: &Logger,
    result: Result<Response<Body>, HttpError>,
) -> Result<Response<Body>, ProtocolError<B>>
where
    B: ErrorBody,
{
    result.or_else(|error| {
        slog::info!(
            log,
            "request failed";
            "response_code" => error.status_code.as_u16(),
            "error_message_internal" => &error.internal_message,
            "error_message_external" => &error.external_message,
        );
        ProtocolError::from(error).into_response()
    })
}

/// An error sent with its protocol's error body at the top level
#[derive(Debug, Serialize, JsonSchema)]
pub struct ProtocolError<B> {
    #[serde(flatten)]
    body: B,
    #[serde(skip)]
    status_code: ErrorStatusCode,
    #[serde(skip)]
    headers: Option<Box<HeaderMap>>,
    #[serde(skip)]
    internal_message: String,
}

impl<B> From<HttpError> for ProtocolError<B>
where
    B: ErrorBody,
{
    fn from(error: HttpError) -> Self {
        let HttpError {
            status_code,
            error_code,
            external_message,
            internal_message,
            headers,
        } = error;
        Self {
            body: B::new(status_code, error_code, external_message),
            status_code,
            headers,
            internal_message,
        }
    }
}

impl<B> fmt::Display for ProtocolError<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.internal_message)
    }
}

impl<B> HttpResponseError for ProtocolError<B>
where
    B: ErrorBody,
{
    fn status_code(&self) -> ErrorStatusCode {
        self.status_code
    }
}

impl<B> ProtocolError<B>
where
    B: ErrorBody,
{
    /// The error as a response, or the error itself if its body fails to serialize
    fn into_response(self) -> Result<Response<Body>, Self> {
        let Ok(body) = serde_json::to_vec(&self) else {
            return Err(self);
        };
        let Self {
            status_code,
            headers,
            ..
        } = self;
        let mut response = Response::new(Body::from(body));
        *response.status_mut() = status_code.as_status();
        if let Some(headers) = headers {
            *response.headers_mut() = *headers;
        }
        response.headers_mut().insert(
            http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        Ok(response)
    }
}

/// The body of a `ProtocolError`
pub trait ErrorBody: Serialize + JsonSchema + Send + Sync + 'static {
    /// The body for an error, which keeps its `error_code` only if the protocol defines it
    fn new(status_code: ErrorStatusCode, error_code: Option<String>, message: String) -> Self;
}

/// The OCI Distribution Spec error body
#[derive(Debug, Serialize, JsonSchema)]
pub struct OciErrors {
    errors: Vec<OciErrorEntry>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct OciErrorEntry {
    code: String,
    message: String,
}

impl ErrorBody for OciErrors {
    fn new(status_code: ErrorStatusCode, error_code: Option<String>, message: String) -> Self {
        let code = error_code
            .filter(|code| OCI_ERROR_CODES.contains(&code.as_str()))
            .unwrap_or_else(|| oci_error_code(status_code).to_owned());
        Self {
            errors: vec![OciErrorEntry { code, message }],
        }
    }
}

fn oci_error_code(status_code: ErrorStatusCode) -> &'static str {
    match status_code.as_status() {
        StatusCode::BAD_REQUEST | StatusCode::METHOD_NOT_ALLOWED | StatusCode::NOT_IMPLEMENTED => {
            OCI_ERROR_UNSUPPORTED
        },
        StatusCode::UNAUTHORIZED => OCI_ERROR_UNAUTHORIZED,
        StatusCode::FORBIDDEN => OCI_ERROR_DENIED,
        StatusCode::NOT_FOUND => OCI_ERROR_NAME_UNKNOWN,
        StatusCode::PAYLOAD_TOO_LARGE => OCI_ERROR_SIZE_INVALID,
        StatusCode::RANGE_NOT_SATISFIABLE => OCI_ERROR_BLOB_UPLOAD_INVALID,
        StatusCode::TOO_MANY_REQUESTS => OCI_ERROR_TOO_MANY_REQUESTS,
        _ => OCI_ERROR_UNKNOWN,
    }
}

pub const OAUTH_ERROR_INVALID_REQUEST: &str = "invalid_request";
pub const OAUTH_ERROR_UNSUPPORTED_GRANT_TYPE: &str = "unsupported_grant_type";
const OAUTH_ERROR_INVALID_CLIENT: &str = "invalid_client";
const OAUTH_ERROR_INVALID_GRANT: &str = "invalid_grant";
const OAUTH_ERROR_UNAUTHORIZED_CLIENT: &str = "unauthorized_client";
const OAUTH_ERROR_INVALID_SCOPE: &str = "invalid_scope";
const OAUTH_ERROR_TEMPORARILY_UNAVAILABLE: &str = "temporarily_unavailable";
const OAUTH_ERROR_SERVER_ERROR: &str = "server_error";

/// Every token endpoint error code in RFC 6749 §5.2, the codes an error may name for itself
const OAUTH_ERROR_CODES: [&str; 6] = [
    OAUTH_ERROR_INVALID_REQUEST,
    OAUTH_ERROR_INVALID_CLIENT,
    OAUTH_ERROR_INVALID_GRANT,
    OAUTH_ERROR_UNAUTHORIZED_CLIENT,
    OAUTH_ERROR_UNSUPPORTED_GRANT_TYPE,
    OAUTH_ERROR_INVALID_SCOPE,
];

/// The `OAuth2` token endpoint error body
#[derive(Debug, Serialize, JsonSchema)]
pub struct OAuthErrorBody {
    error: String,
    error_description: String,
}

impl ErrorBody for OAuthErrorBody {
    fn new(status_code: ErrorStatusCode, error_code: Option<String>, message: String) -> Self {
        let error = error_code
            .filter(|code| OAUTH_ERROR_CODES.contains(&code.as_str()))
            .unwrap_or_else(|| oauth_error_code(status_code).to_owned());
        Self {
            error,
            error_description: message,
        }
    }
}

/// The code for an error that names none.
///
/// RFC 6749 §5.2 has no code for a server error or a rate limit, so those use `server_error`
/// and `temporarily_unavailable`, which §4.1.2.1 defines for the authorization endpoint,
/// as an extension.
fn oauth_error_code(status_code: ErrorStatusCode) -> &'static str {
    if status_code.is_server_error() {
        return OAUTH_ERROR_SERVER_ERROR;
    }
    match status_code.as_status() {
        StatusCode::UNAUTHORIZED => OAUTH_ERROR_INVALID_CLIENT,
        StatusCode::FORBIDDEN => OAUTH_ERROR_INVALID_SCOPE,
        StatusCode::TOO_MANY_REQUESTS => OAUTH_ERROR_TEMPORARILY_UNAVAILABLE,
        _ => OAUTH_ERROR_INVALID_REQUEST,
    }
}
