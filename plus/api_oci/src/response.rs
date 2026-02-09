//! Shared OCI response helpers

/// OCI-specific header for content-addressable digest
pub const DOCKER_CONTENT_DIGEST: &str = "Docker-Content-Digest";
/// OCI-specific header for upload session identifier
pub const DOCKER_UPLOAD_UUID: &str = "Docker-Upload-UUID";
/// OCI-specific header for subject digest (referrers API)
pub const OCI_SUBJECT: &str = "OCI-Subject";
/// OCI-specific header for applied filters (referrers API)
pub const OCI_FILTERS_APPLIED: &str = "OCI-Filters-Applied";
/// Content type for binary blob data
pub const APPLICATION_OCTET_STREAM: &str = "application/octet-stream";
/// Content type for JSON responses
pub const APPLICATION_JSON: &str = "application/json";

/// Adds standard OCI CORS headers to a response builder
///
/// Ensures consistency across all OCI endpoints by centralizing CORS header generation.
pub fn oci_cors_headers(
    builder: http::response::Builder,
    methods: &[http::Method],
) -> http::response::Builder {
    let methods_str = methods
        .iter()
        .map(http::Method::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    builder
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(http::header::ACCESS_CONTROL_ALLOW_METHODS, methods_str)
        .header(
            http::header::ACCESS_CONTROL_ALLOW_HEADERS,
            "Content-Type, Authorization",
        )
}
