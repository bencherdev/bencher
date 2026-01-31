//! HTTP error conversion for OCI errors

use bencher_oci::OciError;
use dropshot::{ClientErrorStatusCode, ErrorStatusCode, HttpError};

/// Converts an `OciError` into an `HttpError`
#[expect(
    clippy::needless_pass_by_value,
    reason = "consumes error for API consistency"
)]
pub fn into_http_error(error: OciError) -> HttpError {
    let status_code = error.status_code();
    let message = error.to_string();
    match status_code {
        http::StatusCode::NOT_FOUND => {
            HttpError::for_client_error(None, ClientErrorStatusCode::NOT_FOUND, message)
        },
        http::StatusCode::BAD_REQUEST => {
            HttpError::for_client_error(None, ClientErrorStatusCode::BAD_REQUEST, message)
        },
        http::StatusCode::UNAUTHORIZED => {
            HttpError::for_client_error(None, ClientErrorStatusCode::UNAUTHORIZED, message)
        },
        http::StatusCode::FORBIDDEN => {
            HttpError::for_client_error(None, ClientErrorStatusCode::FORBIDDEN, message)
        },
        http::StatusCode::TOO_MANY_REQUESTS => {
            HttpError::for_client_error(None, ClientErrorStatusCode::TOO_MANY_REQUESTS, message)
        },
        http::StatusCode::RANGE_NOT_SATISFIABLE => {
            HttpError::for_client_error(None, ClientErrorStatusCode::RANGE_NOT_SATISFIABLE, message)
        },
        http::StatusCode::NOT_IMPLEMENTED => HttpError {
            status_code: ErrorStatusCode::NOT_IMPLEMENTED,
            error_code: None,
            external_message: message.clone(),
            internal_message: message,
            headers: None,
        },
        _ => HttpError {
            status_code: ErrorStatusCode::INTERNAL_SERVER_ERROR,
            error_code: None,
            external_message: message.clone(),
            internal_message: message,
            headers: None,
        },
    }
}
