//! OCI Registry Error Types

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
            Self::BlobUnknown { .. } => "BLOB_UNKNOWN",
            Self::Storage(storage_error) => match storage_error {
                OciStorageError::ManifestNotFound(_) => "MANIFEST_UNKNOWN",
                OciStorageError::DigestMismatch { .. } => "DIGEST_INVALID",
                OciStorageError::UploadNotFound(_) => "BLOB_UPLOAD_UNKNOWN",
                OciStorageError::InvalidContent(_) => "MANIFEST_INVALID",
                OciStorageError::BlobNotFound(_)
                | OciStorageError::S3(_)
                | OciStorageError::LocalStorage(_)
                | OciStorageError::InvalidArn(_)
                | OciStorageError::Config(_)
                | OciStorageError::Json(_) => "BLOB_UNKNOWN",
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_error_codes() {
        // Each OciStorageError variant should map to the correct OCI error code
        assert_eq!(
            OciError::from(OciStorageError::ManifestNotFound("m".into())).code(),
            "MANIFEST_UNKNOWN"
        );
        assert_eq!(
            OciError::from(OciStorageError::DigestMismatch {
                expected: "a".into(),
                actual: "b".into()
            })
            .code(),
            "DIGEST_INVALID"
        );
        assert_eq!(
            OciError::from(OciStorageError::UploadNotFound("u".into())).code(),
            "BLOB_UPLOAD_UNKNOWN"
        );
        assert_eq!(
            OciError::from(OciStorageError::InvalidContent("c".into())).code(),
            "MANIFEST_INVALID"
        );
        assert_eq!(
            OciError::from(OciStorageError::BlobNotFound("b".into())).code(),
            "BLOB_UNKNOWN"
        );
        assert_eq!(
            OciError::from(OciStorageError::S3("s3".into())).code(),
            "BLOB_UNKNOWN"
        );
        assert_eq!(
            OciError::from(OciStorageError::LocalStorage("fs".into())).code(),
            "BLOB_UNKNOWN"
        );
        assert_eq!(
            OciError::from(OciStorageError::Json("json".into())).code(),
            "BLOB_UNKNOWN"
        );
    }

    #[test]
    fn direct_error_codes() {
        assert_eq!(
            OciError::BlobUnknown { digest: "d".into() }.code(),
            "BLOB_UNKNOWN"
        );
        assert_eq!(
            OciError::ManifestUnknown {
                reference: "r".into()
            }
            .code(),
            "MANIFEST_UNKNOWN"
        );
        assert_eq!(
            OciError::NameInvalid { name: "n".into() }.code(),
            "NAME_INVALID"
        );
        assert_eq!(OciError::Unauthorized("u".into()).code(), "UNAUTHORIZED");
        assert_eq!(OciError::TooManyRequests.code(), "TOOMANYREQUESTS");
    }
}
