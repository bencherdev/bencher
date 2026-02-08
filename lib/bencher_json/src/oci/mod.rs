mod manifest;

pub use manifest::{
    DockerManifestList, DockerManifestV2, Manifest, OCI_IMAGE_INDEX_MEDIA_TYPE, OciDescriptor,
    OciImageIndex, OciImageManifest, OciManifestDescriptor, Platform,
};

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
