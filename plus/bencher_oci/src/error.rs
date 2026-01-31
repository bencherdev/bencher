//! OCI Registry Error Types

use dropshot::{ClientErrorStatusCode, ErrorStatusCode, HttpError};
use thiserror::Error;

/// OCI-specific errors
#[derive(Debug, Error)]
pub enum OciError {
    #[error("Blob unknown: {digest}")]
    BlobUnknown { digest: String },

    #[error("Blob upload invalid: {upload_id}")]
    BlobUploadInvalid { upload_id: String },

    #[error("Blob upload unknown: {upload_id}")]
    BlobUploadUnknown { upload_id: String },

    #[error("Digest invalid: {digest}")]
    DigestInvalid { digest: String },

    #[error("Manifest blob unknown: {digest}")]
    ManifestBlobUnknown { digest: String },

    #[error("Manifest invalid: {0}")]
    ManifestInvalid(String),

    #[error("Manifest unknown: {reference}")]
    ManifestUnknown { reference: String },

    #[error("Name invalid: {name}")]
    NameInvalid { name: String },

    #[error("Name unknown: {name}")]
    NameUnknown { name: String },

    #[error("Size invalid: {0}")]
    SizeInvalid(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Denied: {0}")]
    Denied(String),

    #[error("Unsupported: {0}")]
    Unsupported(String),

    #[error("Too many requests")]
    TooManyRequests,

    #[error("Range not satisfiable: {0}")]
    RangeNotSatisfiable(String),

    #[error("Storage error: {0}")]
    Storage(#[from] crate::storage::OciStorageError),
}

impl OciError {
    /// Returns the OCI error code as specified in the Distribution Spec
    pub fn code(&self) -> &'static str {
        match self {
            Self::BlobUnknown { .. } | Self::Storage(_) => "BLOB_UNKNOWN",
            Self::BlobUploadInvalid { .. } | Self::RangeNotSatisfiable(_) => "BLOB_UPLOAD_INVALID",
            Self::BlobUploadUnknown { .. } => "BLOB_UPLOAD_UNKNOWN",
            Self::DigestInvalid { .. } => "DIGEST_INVALID",
            Self::ManifestBlobUnknown { .. } => "MANIFEST_BLOB_UNKNOWN",
            Self::ManifestInvalid(_) => "MANIFEST_INVALID",
            Self::ManifestUnknown { .. } => "MANIFEST_UNKNOWN",
            Self::NameInvalid { .. } => "NAME_INVALID",
            Self::NameUnknown { .. } => "NAME_UNKNOWN",
            Self::SizeInvalid(_) => "SIZE_INVALID",
            Self::Unauthorized(_) => "UNAUTHORIZED",
            Self::Denied(_) => "DENIED",
            Self::Unsupported(_) => "UNSUPPORTED",
            Self::TooManyRequests => "TOOMANYREQUESTS",
        }
    }

    /// Returns the appropriate HTTP status code for this error
    pub fn status_code(&self) -> http::StatusCode {
        match self {
            Self::BlobUnknown { .. }
            | Self::BlobUploadUnknown { .. }
            | Self::ManifestUnknown { .. }
            | Self::NameUnknown { .. } => http::StatusCode::NOT_FOUND,

            Self::BlobUploadInvalid { .. }
            | Self::DigestInvalid { .. }
            | Self::ManifestBlobUnknown { .. }
            | Self::ManifestInvalid(_)
            | Self::NameInvalid { .. }
            | Self::SizeInvalid(_) => http::StatusCode::BAD_REQUEST,

            Self::Unauthorized(_) => http::StatusCode::UNAUTHORIZED,

            Self::Denied(_) => http::StatusCode::FORBIDDEN,

            Self::Unsupported(_) => http::StatusCode::NOT_IMPLEMENTED,

            Self::TooManyRequests => http::StatusCode::TOO_MANY_REQUESTS,

            Self::RangeNotSatisfiable(_) => http::StatusCode::RANGE_NOT_SATISFIABLE,

            Self::Storage(storage_error) => storage_error.status_code(),
        }
    }
}

impl From<OciError> for HttpError {
    fn from(error: OciError) -> Self {
        let message = error.to_string();
        match error.status_code() {
            http::StatusCode::NOT_FOUND => HttpError::for_client_error(
                None,
                ClientErrorStatusCode::NOT_FOUND,
                message,
            ),
            http::StatusCode::BAD_REQUEST => HttpError::for_client_error(
                None,
                ClientErrorStatusCode::BAD_REQUEST,
                message,
            ),
            http::StatusCode::UNAUTHORIZED => HttpError::for_client_error(
                None,
                ClientErrorStatusCode::UNAUTHORIZED,
                message,
            ),
            http::StatusCode::FORBIDDEN => HttpError::for_client_error(
                None,
                ClientErrorStatusCode::FORBIDDEN,
                message,
            ),
            http::StatusCode::TOO_MANY_REQUESTS => HttpError::for_client_error(
                None,
                ClientErrorStatusCode::TOO_MANY_REQUESTS,
                message,
            ),
            http::StatusCode::RANGE_NOT_SATISFIABLE => HttpError::for_client_error(
                None,
                ClientErrorStatusCode::RANGE_NOT_SATISFIABLE,
                message,
            ),
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
}
