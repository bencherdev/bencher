mod manifest;

pub use manifest::{
    DockerManifestList, DockerManifestV2, Manifest, OCI_IMAGE_INDEX_MEDIA_TYPE, OciDescriptor,
    OciImageIndex, OciImageManifest, OciManifestDescriptor, Platform,
};

// OCI Distribution Spec error codes
// https://github.com/opencontainers/distribution-spec/blob/main/spec.md#error-codes
pub const OCI_ERROR_BLOB_UNKNOWN: &str = "BLOB_UNKNOWN";
pub const OCI_ERROR_BLOB_UPLOAD_INVALID: &str = "BLOB_UPLOAD_INVALID";
pub const OCI_ERROR_BLOB_UPLOAD_UNKNOWN: &str = "BLOB_UPLOAD_UNKNOWN";
pub const OCI_ERROR_DENIED: &str = "DENIED";
pub const OCI_ERROR_DIGEST_INVALID: &str = "DIGEST_INVALID";
pub const OCI_ERROR_MANIFEST_BLOB_UNKNOWN: &str = "MANIFEST_BLOB_UNKNOWN";
pub const OCI_ERROR_MANIFEST_INVALID: &str = "MANIFEST_INVALID";
pub const OCI_ERROR_MANIFEST_UNKNOWN: &str = "MANIFEST_UNKNOWN";
pub const OCI_ERROR_NAME_INVALID: &str = "NAME_INVALID";
pub const OCI_ERROR_NAME_UNKNOWN: &str = "NAME_UNKNOWN";
pub const OCI_ERROR_SIZE_INVALID: &str = "SIZE_INVALID";
pub const OCI_ERROR_TAG_INVALID: &str = "TAG_INVALID";
pub const OCI_ERROR_TOO_MANY_REQUESTS: &str = "TOOMANYREQUESTS";
pub const OCI_ERROR_UNAUTHORIZED: &str = "UNAUTHORIZED";
pub const OCI_ERROR_UNKNOWN: &str = "UNKNOWN";
pub const OCI_ERROR_UNSUPPORTED: &str = "UNSUPPORTED";

/// Every error code above: the OCI Distribution Spec's codes, plus `TAG_INVALID` and `UNKNOWN`
/// from the error codes of the `distribution` project
pub const OCI_ERROR_CODES: [&str; 16] = [
    OCI_ERROR_BLOB_UNKNOWN,
    OCI_ERROR_BLOB_UPLOAD_INVALID,
    OCI_ERROR_BLOB_UPLOAD_UNKNOWN,
    OCI_ERROR_DENIED,
    OCI_ERROR_DIGEST_INVALID,
    OCI_ERROR_MANIFEST_BLOB_UNKNOWN,
    OCI_ERROR_MANIFEST_INVALID,
    OCI_ERROR_MANIFEST_UNKNOWN,
    OCI_ERROR_NAME_INVALID,
    OCI_ERROR_NAME_UNKNOWN,
    OCI_ERROR_SIZE_INVALID,
    OCI_ERROR_TAG_INVALID,
    OCI_ERROR_TOO_MANY_REQUESTS,
    OCI_ERROR_UNAUTHORIZED,
    OCI_ERROR_UNKNOWN,
    OCI_ERROR_UNSUPPORTED,
];
