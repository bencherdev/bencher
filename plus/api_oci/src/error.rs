//! HTTP error conversion for OCI errors

pub use api_auth::oci::{RegistryError, protocol_response};
use bencher_json::oci::OCI_ERROR_SIZE_INVALID;
use bencher_oci_storage::OciError;
use dropshot::{ErrorStatusCode, HttpError};

/// Converts an `OciStorageError` into an `HttpError` via `OciError`
pub fn storage_error(error: bencher_oci_storage::OciStorageError) -> HttpError {
    into_http_error(OciError::from(error))
}

/// Converts an `OciError` into an `HttpError` with its OCI error code
#[expect(
    clippy::needless_pass_by_value,
    reason = "consumes error for API consistency"
)]
pub fn into_http_error(error: OciError) -> HttpError {
    let message = error.to_string();
    if let Ok(status_code) = ErrorStatusCode::from_status(error.status_code())
        && !status_code.is_server_error()
    {
        HttpError {
            status_code,
            error_code: Some(error.code().to_owned()),
            external_message: message.clone(),
            internal_message: message,
            headers: None,
        }
    } else {
        HttpError::for_internal_error(message)
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

/// Returns a 413 Payload Too Large error with its OCI error code.
///
/// Dropshot's `ClientErrorStatusCode` does not include 413,
/// so we construct the `HttpError` manually.
pub fn payload_too_large(size: u64, max: u64) -> HttpError {
    let message = format!("Payload size {size} bytes exceeds maximum allowed {max} bytes");
    HttpError {
        status_code: ErrorStatusCode::PAYLOAD_TOO_LARGE,
        error_code: Some(OCI_ERROR_SIZE_INVALID.to_owned()),
        external_message: message.clone(),
        internal_message: message,
        headers: None,
    }
}

#[cfg(test)]
mod tests {
    use bencher_oci_storage::{OciError, OciStorageError};
    use dropshot::ErrorStatusCode;
    use serde_json::json;

    use super::{RegistryError, into_http_error};

    #[test]
    fn into_http_error_storage_is_generic() {
        assert_generic(OciError::Storage(OciStorageError::Json(
            "secret storage detail".to_owned(),
        )));
    }

    #[test]
    fn into_http_error_server_error_is_generic() {
        assert_generic(OciError::Unsupported("secret storage detail".to_owned()));
    }

    fn assert_generic(error: OciError) {
        let http_error = into_http_error(error);
        assert_eq!(
            http_error.status_code,
            ErrorStatusCode::INTERNAL_SERVER_ERROR
        );
        assert!(
            http_error
                .internal_message
                .contains("secret storage detail")
        );
        let body = serde_json::to_value(RegistryError::from(http_error)).unwrap();
        assert_eq!(
            body,
            json!({"errors": [{"code": "UNKNOWN", "message": "Internal Server Error"}]})
        );
    }
}
