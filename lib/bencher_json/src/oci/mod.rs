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

/// Formats an OCI-compliant JSON error body
///
/// Per the OCI Distribution Spec, if the response body is JSON it MUST follow:
/// `{"errors": [{"code": "<CODE>", "message": "<msg>"}]}`
pub fn oci_error_body(code: &str, message: &str) -> String {
    serde_json::json!({
        "errors": [{
            "code": code,
            "message": message
        }]
    })
    .to_string()
}
