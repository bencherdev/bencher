//! HTTP error conversion for OCI errors

use bencher_json::oci::{OCI_ERROR_SIZE_INVALID, OCI_ERROR_UNKNOWN, oci_error_body};
use bencher_oci_storage::OciError;
use dropshot::{ClientErrorStatusCode, ErrorStatusCode, HttpError};

/// Converts an `OciError` into an `HttpError` with OCI-compliant JSON error body
#[expect(
    clippy::needless_pass_by_value,
    reason = "consumes error for API consistency"
)]
pub fn into_http_error(error: OciError) -> HttpError {
    let status_code = error.status_code();
    let code = error.code();
    let message = error.to_string();
    let external_message = oci_error_body(code, &message);
    match status_code {
        http::StatusCode::NOT_FOUND => {
            HttpError::for_client_error(None, ClientErrorStatusCode::NOT_FOUND, external_message)
        },
        http::StatusCode::BAD_REQUEST => {
            HttpError::for_client_error(None, ClientErrorStatusCode::BAD_REQUEST, external_message)
        },
        http::StatusCode::UNAUTHORIZED => {
            HttpError::for_client_error(None, ClientErrorStatusCode::UNAUTHORIZED, external_message)
        },
        http::StatusCode::FORBIDDEN => {
            HttpError::for_client_error(None, ClientErrorStatusCode::FORBIDDEN, external_message)
        },
        http::StatusCode::TOO_MANY_REQUESTS => HttpError::for_client_error(
            None,
            ClientErrorStatusCode::TOO_MANY_REQUESTS,
            external_message,
        ),
        http::StatusCode::PAYLOAD_TOO_LARGE => HttpError {
            status_code: ErrorStatusCode::PAYLOAD_TOO_LARGE,
            error_code: None,
            external_message,
            internal_message: message,
            headers: None,
        },
        http::StatusCode::RANGE_NOT_SATISFIABLE => HttpError::for_client_error(
            None,
            ClientErrorStatusCode::RANGE_NOT_SATISFIABLE,
            external_message,
        ),
        http::StatusCode::NOT_IMPLEMENTED => HttpError {
            status_code: ErrorStatusCode::NOT_IMPLEMENTED,
            error_code: None,
            external_message,
            internal_message: message,
            headers: None,
        },
        _ => HttpError {
            status_code: ErrorStatusCode::INTERNAL_SERVER_ERROR,
            error_code: None,
            external_message: oci_error_body(OCI_ERROR_UNKNOWN, "Internal server error"),
            internal_message: message,
            headers: None,
        },
    }
}

/// Parses a digest string, returning an OCI-compliant error on failure
pub fn parse_digest(s: &str) -> Result<bencher_oci_storage::Digest, HttpError> {
    s.parse().map_err(|_e| {
        into_http_error(OciError::DigestInvalid {
            digest: s.to_owned(),
        })
    })
}

/// Parses an upload ID string, returning an OCI-compliant error on failure
pub fn parse_upload_id(s: &str) -> Result<bencher_oci_storage::UploadId, HttpError> {
    s.parse().map_err(|_e| {
        into_http_error(OciError::BlobUploadUnknown {
            upload_id: s.to_owned(),
        })
    })
}

/// Returns a 413 Payload Too Large error with OCI-compliant JSON body.
///
/// Dropshot's `ClientErrorStatusCode` does not include 413,
/// so we construct the `HttpError` manually.
pub fn payload_too_large(size: u64, max: u64) -> HttpError {
    let message = format!("Payload size {size} bytes exceeds maximum allowed {max} bytes");
    HttpError {
        status_code: ErrorStatusCode::PAYLOAD_TOO_LARGE,
        error_code: None,
        external_message: oci_error_body(OCI_ERROR_SIZE_INVALID, &message),
        internal_message: message,
        headers: None,
    }
}
