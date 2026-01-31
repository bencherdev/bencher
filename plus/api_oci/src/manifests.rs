//! OCI Manifest Endpoints
//!
//! - HEAD /v2/<name>/manifests/<reference> - Check manifest existence
//! - GET /v2/<name>/manifests/<reference> - Download manifest
//! - PUT /v2/<name>/manifests/<reference> - Upload manifest
//! - DELETE /v2/<name>/manifests/<reference> - Delete manifest

use bencher_endpoint::{CorsResponse, Delete, Endpoint, Get, Put};
use bencher_oci_storage::{OciError, Reference, RepositoryName};
use bencher_schema::context::ApiContext;
use dropshot::{Body, HttpError, Path, RequestContext, UntypedBody, endpoint};
use http::Response;
use schemars::JsonSchema;
use serde::Deserialize;

/// Path parameters for manifest endpoints
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ManifestPath {
    /// Repository name (e.g., "library/ubuntu")
    pub name: String,
    /// Reference (tag or digest)
    pub reference: String,
}

/// CORS preflight for manifest endpoints
#[endpoint {
    method = OPTIONS,
    path = "/v2/{name}/manifests/{reference}",
    tags = ["oci"],
}]
pub async fn oci_manifest_options(
    _rqctx: RequestContext<ApiContext>,
    _path: Path<ManifestPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Put.into(), Delete.into()]))
}

/// Check if a manifest exists
#[endpoint {
    method = HEAD,
    path = "/v2/{name}/manifests/{reference}",
    tags = ["oci"],
}]
pub async fn oci_manifest_exists(
    rqctx: RequestContext<ApiContext>,
    path: Path<ManifestPath>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();

    // Parse and validate inputs
    let repository: RepositoryName = path
        .name
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::NameInvalid { name: path.name.clone() }))?;
    let reference: Reference = path
        .reference
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::ManifestUnknown { reference: path.reference.clone() }))?;

    // Get storage
    let storage = rqctx.context().oci_storage()?;

    // Resolve the reference to a digest
    let digest = match &reference {
        Reference::Digest(d) => d.clone(),
        Reference::Tag(t) => storage
            .resolve_tag(&repository, t.as_str())
            .await
            .map_err(|e| crate::error::into_http_error(OciError::from(e)))?,
    };

    // Get manifest to check existence and get size
    let manifest = storage
        .get_manifest_by_digest(&repository, &digest)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Determine content type (default to OCI manifest)
    let content_type = "application/vnd.oci.image.manifest.v1+json";

    // Build response with OCI-compliant headers (no body for HEAD)
    let response = Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, content_type)
        .header(http::header::CONTENT_LENGTH, manifest.len())
        .header("Docker-Content-Digest", digest.to_string())
        // CORS headers
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(http::header::ACCESS_CONTROL_ALLOW_METHODS, "HEAD, GET")
        .header(http::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .body(Body::empty())
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Download a manifest
#[endpoint {
    method = GET,
    path = "/v2/{name}/manifests/{reference}",
    tags = ["oci"],
}]
pub async fn oci_manifest_get(
    rqctx: RequestContext<ApiContext>,
    path: Path<ManifestPath>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();

    // Parse and validate inputs
    let repository: RepositoryName = path
        .name
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::NameInvalid { name: path.name.clone() }))?;
    let reference: Reference = path
        .reference
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::ManifestUnknown { reference: path.reference.clone() }))?;

    // Get storage
    let storage = rqctx.context().oci_storage()?;

    // Resolve the reference to a digest
    let digest = match &reference {
        Reference::Digest(d) => d.clone(),
        Reference::Tag(t) => storage
            .resolve_tag(&repository, t.as_str())
            .await
            .map_err(|e| crate::error::into_http_error(OciError::from(e)))?,
    };

    // Get manifest content
    let manifest = storage
        .get_manifest_by_digest(&repository, &digest)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciManifestPull);

    // Determine content type from manifest (default to OCI manifest)
    // In a full implementation, we'd parse the manifest to determine the media type
    let content_type = "application/vnd.oci.image.manifest.v1+json";

    // Build response with OCI-compliant headers
    let response = Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, content_type)
        .header(http::header::CONTENT_LENGTH, manifest.len())
        .header("Docker-Content-Digest", digest.to_string())
        // CORS headers
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(http::header::ACCESS_CONTROL_ALLOW_METHODS, "GET")
        .header(http::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .body(Body::from(manifest))
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Upload a manifest
#[endpoint {
    method = PUT,
    path = "/v2/{name}/manifests/{reference}",
    tags = ["oci"],
}]
pub async fn oci_manifest_put(
    rqctx: RequestContext<ApiContext>,
    path: Path<ManifestPath>,
    body: UntypedBody,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let content = body.as_bytes();

    // Parse and validate inputs
    let repository: RepositoryName = path
        .name
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::NameInvalid { name: path.name.clone() }))?;
    let reference: Reference = path
        .reference
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::ManifestInvalid(path.reference.clone())))?;

    // Get storage
    let storage = rqctx.context().oci_storage()?;

    // Determine tag from reference (if it's a tag)
    let tag = match &reference {
        Reference::Tag(t) => Some(t.as_str()),
        Reference::Digest(_) => None,
    };

    // Extract subject digest from manifest if present (for OCI-Subject header)
    let subject_digest = serde_json::from_slice::<serde_json::Value>(content)
        .ok()
        .and_then(|manifest| {
            manifest
                .get("subject")
                .and_then(|s| s.get("digest"))
                .and_then(|d| d.as_str())
                .map(ToOwned::to_owned)
        });

    // Store the manifest
    let digest = storage
        .put_manifest(&repository, bytes::Bytes::copy_from_slice(content), tag)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciManifestPush);

    // Build 201 Created response with Location and Docker-Content-Digest headers
    let location = format!("/v2/{repository}/manifests/{digest}");

    let mut builder = Response::builder()
        .status(http::StatusCode::CREATED)
        .header(http::header::LOCATION, location)
        .header("Docker-Content-Digest", digest.to_string())
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(http::header::ACCESS_CONTROL_ALLOW_METHODS, "PUT")
        .header(http::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type, Authorization");

    // Add OCI-Subject header if manifest has a subject field
    if let Some(subject) = subject_digest {
        builder = builder.header("OCI-Subject", subject);
    }

    let response = builder
        .body(Body::empty())
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Delete a manifest
#[endpoint {
    method = DELETE,
    path = "/v2/{name}/manifests/{reference}",
    tags = ["oci"],
}]
pub async fn oci_manifest_delete(
    rqctx: RequestContext<ApiContext>,
    path: Path<ManifestPath>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();

    // Parse and validate inputs
    let repository: RepositoryName = path
        .name
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::NameInvalid { name: path.name.clone() }))?;

    // Get storage
    let storage = rqctx.context().oci_storage()?;

    // Parse reference - can be either a digest or a tag
    let reference: Reference = path
        .reference
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::ManifestUnknown { reference: path.reference.clone() }))?;

    match reference {
        Reference::Digest(digest) => {
            // Delete by digest - delete the manifest itself
            storage
                .delete_manifest(&repository, &digest)
                .await
                .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;
        }
        Reference::Tag(tag) => {
            // Delete by tag - delete the tag link only (manifest may still exist)
            storage
                .delete_tag(&repository, tag.as_str())
                .await
                .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;
        }
    }

    // OCI spec requires 202 Accepted for DELETE
    let response = Response::builder()
        .status(http::StatusCode::ACCEPTED)
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::empty())
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}
