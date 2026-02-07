//! Shared OCI response helpers

/// Adds standard OCI CORS headers to a response builder
///
/// Ensures consistency across all OCI endpoints by centralizing CORS header generation.
pub fn oci_cors_headers(
    builder: http::response::Builder,
    methods: &str,
) -> http::response::Builder {
    builder
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(http::header::ACCESS_CONTROL_ALLOW_METHODS, methods)
        .header(
            http::header::ACCESS_CONTROL_ALLOW_HEADERS,
            "Content-Type, Authorization",
        )
}
