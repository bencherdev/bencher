//! OCI Registry Error Types

use bencher_json::oci::{
    OCI_ERROR_BLOB_UNKNOWN, OCI_ERROR_BLOB_UPLOAD_INVALID, OCI_ERROR_BLOB_UPLOAD_UNKNOWN,
    OCI_ERROR_DENIED, OCI_ERROR_DIGEST_INVALID, OCI_ERROR_MANIFEST_BLOB_UNKNOWN,
    OCI_ERROR_MANIFEST_INVALID, OCI_ERROR_MANIFEST_UNKNOWN, OCI_ERROR_NAME_INVALID,
    OCI_ERROR_NAME_UNKNOWN, OCI_ERROR_SIZE_INVALID, OCI_ERROR_TAG_INVALID,
    OCI_ERROR_TOO_MANY_REQUESTS, OCI_ERROR_UNAUTHORIZED, OCI_ERROR_UNKNOWN, OCI_ERROR_UNSUPPORTED,
};
use thiserror::Error;

use crate::storage::OciStorageError;

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

    #[error("Tag invalid: {tag}")]
    TagInvalid { tag: String },

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
    Storage(#[from] OciStorageError),
}

impl OciError {
    /// Returns the OCI error code as specified in the Distribution Spec
    pub fn code(&self) -> &'static str {
        match self {
            Self::BlobUnknown { .. } => OCI_ERROR_BLOB_UNKNOWN,
            Self::Storage(storage_error) => match storage_error {
                OciStorageError::ManifestNotFound(_) => OCI_ERROR_MANIFEST_UNKNOWN,
                OciStorageError::DigestMismatch { .. } => OCI_ERROR_DIGEST_INVALID,
                OciStorageError::UploadNotFound(_) => OCI_ERROR_BLOB_UPLOAD_UNKNOWN,
                OciStorageError::InvalidContent(_) => OCI_ERROR_MANIFEST_INVALID,
                OciStorageError::BlobUploadInvalidContent(_) => OCI_ERROR_BLOB_UPLOAD_INVALID,
                OciStorageError::BlobNotFound(_) => OCI_ERROR_BLOB_UNKNOWN,
                OciStorageError::SizeExceeded { .. } => OCI_ERROR_SIZE_INVALID,
                OciStorageError::S3(_)
                | OciStorageError::LocalStorage(_)
                | OciStorageError::InvalidArn(_)
                | OciStorageError::Config(_)
                | OciStorageError::Json(_) => OCI_ERROR_UNKNOWN,
            },
            Self::BlobUploadInvalid { .. } | Self::RangeNotSatisfiable(_) => {
                OCI_ERROR_BLOB_UPLOAD_INVALID
            },
            Self::BlobUploadUnknown { .. } => OCI_ERROR_BLOB_UPLOAD_UNKNOWN,
            Self::DigestInvalid { .. } => OCI_ERROR_DIGEST_INVALID,
            Self::ManifestBlobUnknown { .. } => OCI_ERROR_MANIFEST_BLOB_UNKNOWN,
            Self::ManifestInvalid(_) => OCI_ERROR_MANIFEST_INVALID,
            Self::ManifestUnknown { .. } => OCI_ERROR_MANIFEST_UNKNOWN,
            Self::NameInvalid { .. } => OCI_ERROR_NAME_INVALID,
            Self::NameUnknown { .. } => OCI_ERROR_NAME_UNKNOWN,
            Self::TagInvalid { .. } => OCI_ERROR_TAG_INVALID,
            Self::SizeInvalid(_) => OCI_ERROR_SIZE_INVALID,
            Self::Unauthorized(_) => OCI_ERROR_UNAUTHORIZED,
            Self::Denied(_) => OCI_ERROR_DENIED,
            Self::Unsupported(_) => OCI_ERROR_UNSUPPORTED,
            Self::TooManyRequests => OCI_ERROR_TOO_MANY_REQUESTS,
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
            | Self::TagInvalid { .. }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_error_codes() {
        // Each OciStorageError variant should map to the correct OCI error code
        assert_eq!(
            OciError::from(OciStorageError::ManifestNotFound("m".into())).code(),
            OCI_ERROR_MANIFEST_UNKNOWN
        );
        assert_eq!(
            OciError::from(OciStorageError::DigestMismatch {
                expected: "a".into(),
                actual: "b".into()
            })
            .code(),
            OCI_ERROR_DIGEST_INVALID
        );
        assert_eq!(
            OciError::from(OciStorageError::UploadNotFound("u".into())).code(),
            OCI_ERROR_BLOB_UPLOAD_UNKNOWN
        );
        assert_eq!(
            OciError::from(OciStorageError::InvalidContent("c".into())).code(),
            OCI_ERROR_MANIFEST_INVALID
        );
        assert_eq!(
            OciError::from(OciStorageError::BlobNotFound("b".into())).code(),
            OCI_ERROR_BLOB_UNKNOWN
        );
        assert_eq!(
            OciError::from(OciStorageError::SizeExceeded { size: 100, max: 50 }).code(),
            OCI_ERROR_SIZE_INVALID
        );
        assert_eq!(
            OciError::from(OciStorageError::S3("s3".into())).code(),
            OCI_ERROR_UNKNOWN
        );
        assert_eq!(
            OciError::from(OciStorageError::LocalStorage("fs".into())).code(),
            OCI_ERROR_UNKNOWN
        );
        assert_eq!(
            OciError::from(OciStorageError::Json("json".into())).code(),
            OCI_ERROR_UNKNOWN
        );
    }

    #[test]
    fn direct_error_codes() {
        assert_eq!(
            OciError::BlobUnknown { digest: "d".into() }.code(),
            OCI_ERROR_BLOB_UNKNOWN
        );
        assert_eq!(
            OciError::ManifestUnknown {
                reference: "r".into()
            }
            .code(),
            OCI_ERROR_MANIFEST_UNKNOWN
        );
        assert_eq!(
            OciError::NameInvalid { name: "n".into() }.code(),
            OCI_ERROR_NAME_INVALID
        );
        assert_eq!(
            OciError::TagInvalid { tag: "t".into() }.code(),
            OCI_ERROR_TAG_INVALID
        );
        assert_eq!(
            OciError::Unauthorized("u".into()).code(),
            OCI_ERROR_UNAUTHORIZED
        );
        assert_eq!(
            OciError::TooManyRequests.code(),
            OCI_ERROR_TOO_MANY_REQUESTS
        );
    }
}
