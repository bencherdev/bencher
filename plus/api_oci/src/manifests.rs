//! OCI Manifest Endpoints
//!
//! - HEAD /v2/<name>/manifests/<reference> - Check manifest existence
//! - GET /v2/<name>/manifests/<reference> - Download manifest
//! - PUT /v2/<name>/manifests/<reference> - Upload manifest
//! - DELETE /v2/<name>/manifests/<reference> - Delete manifest

use bencher_endpoint::{CorsResponse, Delete, Endpoint, Get, Put};
use bencher_json::ProjectResourceId;
use bencher_oci_storage::{OciError, Reference};
use bencher_schema::context::ApiContext;
use dropshot::{Body, HttpError, Path, RequestContext, UntypedBody, endpoint};
use http::Response;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::auth::{extract_oci_bearer_token, unauthorized_with_www_authenticate, validate_oci_access, validate_push_access};

/// Path parameters for manifest endpoints
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ManifestPath {
    /// Project resource ID (UUID or slug)
    pub name: ProjectResourceId,
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
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding auth errors for security"
)]
#[endpoint {
    method = HEAD,
    path = "/v2/{name}/manifests/{reference}",
    tags = ["oci"],
}]
pub async fn oci_manifest_exists(
    rqctx: RequestContext<ApiContext>,
    path: Path<ManifestPath>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();

    // Authenticate
    let name_str = path.name.to_string();
    let scope = format!("repository:{name_str}:pull");
    let token = extract_oci_bearer_token(&rqctx)
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;
    validate_oci_access(context, &token, &name_str, "pull")
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;

    // Parse reference
    let reference: Reference = path
        .reference
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::ManifestUnknown { reference: path.reference.clone() }))?;

    // Get storage
    let storage = context.oci_storage()?;

    // Resolve the reference to a digest
    let digest = match &reference {
        Reference::Digest(d) => d.clone(),
        Reference::Tag(t) => storage
            .resolve_tag(&path.name, t.as_str())
            .await
            .map_err(|e| crate::error::into_http_error(OciError::from(e)))?,
    };

    // Get manifest to check existence and get size
    let manifest = storage
        .get_manifest_by_digest(&path.name, &digest)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Determine content type from manifest (parse to get mediaType field)
    let content_type = serde_json::from_slice::<serde_json::Value>(&manifest)
        .ok()
        .and_then(|v| v.get("mediaType").and_then(|m| m.as_str()).map(ToOwned::to_owned))
        .unwrap_or_else(|| "application/vnd.oci.image.manifest.v1+json".to_owned());

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
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding auth errors for security"
)]
#[endpoint {
    method = GET,
    path = "/v2/{name}/manifests/{reference}",
    tags = ["oci"],
}]
pub async fn oci_manifest_get(
    rqctx: RequestContext<ApiContext>,
    path: Path<ManifestPath>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();

    // Authenticate
    let name_str = path.name.to_string();
    let scope = format!("repository:{name_str}:pull");
    let token = extract_oci_bearer_token(&rqctx)
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;
    validate_oci_access(context, &token, &name_str, "pull")
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;

    // Parse reference
    let reference: Reference = path
        .reference
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::ManifestUnknown { reference: path.reference.clone() }))?;

    // Get storage
    let storage = context.oci_storage()?;

    // Resolve the reference to a digest
    let digest = match &reference {
        Reference::Digest(d) => d.clone(),
        Reference::Tag(t) => storage
            .resolve_tag(&path.name, t.as_str())
            .await
            .map_err(|e| crate::error::into_http_error(OciError::from(e)))?,
    };

    // Get manifest content
    let manifest = storage
        .get_manifest_by_digest(&path.name, &digest)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciManifestPull);

    // Determine content type from manifest (parse to get mediaType field)
    let content_type = serde_json::from_slice::<serde_json::Value>(&manifest)
        .ok()
        .and_then(|v| v.get("mediaType").and_then(|m| m.as_str()).map(ToOwned::to_owned))
        .unwrap_or_else(|| "application/vnd.oci.image.manifest.v1+json".to_owned());

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
///
/// Authentication is optional for unclaimed projects.
/// If the project's organization is claimed, valid authentication with Create permission is required.
/// If the project doesn't exist and a slug is used, the project will be created automatically.
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
    let context = rqctx.context();
    let path = path.into_inner();
    let body_bytes = body.as_bytes();

    // Validate push access and get or create the project
    let push_access = validate_push_access(&rqctx.log, &rqctx, &path.name).await?;
    let project_slug = &push_access.project.slug;

    // Parse reference
    let reference: Reference = path
        .reference
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::ManifestInvalid(path.reference.clone())))?;

    // Get storage
    let storage = context.oci_storage()?;

    // Determine tag from reference (if it's a tag)
    let tag = match &reference {
        Reference::Tag(t) => Some(t.as_str()),
        Reference::Digest(_) => None,
    };

    // Extract subject digest from manifest if present (for OCI-Subject header)
    let subject_digest = serde_json::from_slice::<serde_json::Value>(body_bytes)
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
        .put_manifest(&path.name, bytes::Bytes::copy_from_slice(body_bytes), tag)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciManifestPush);

    // Build 201 Created response with Location and Docker-Content-Digest headers
    let location = format!("/v2/{project_slug}/manifests/{digest}");

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
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding auth errors for security"
)]
#[endpoint {
    method = DELETE,
    path = "/v2/{name}/manifests/{reference}",
    tags = ["oci"],
}]
pub async fn oci_manifest_delete(
    rqctx: RequestContext<ApiContext>,
    path: Path<ManifestPath>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();

    // Authenticate (delete requires push permission)
    let name_str = path.name.to_string();
    let scope = format!("repository:{name_str}:push");
    let token = extract_oci_bearer_token(&rqctx)
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;
    validate_oci_access(context, &token, &name_str, "push")
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;

    // Get storage
    let storage = context.oci_storage()?;

    // Parse reference - can be either a digest or a tag
    let reference: Reference = path
        .reference
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::ManifestUnknown { reference: path.reference.clone() }))?;

    match reference {
        Reference::Digest(digest) => {
            // Delete by digest - delete the manifest itself
            storage
                .delete_manifest(&path.name, &digest)
                .await
                .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;
        }
        Reference::Tag(tag) => {
            // Delete by tag - delete the tag link only (manifest may still exist)
            storage
                .delete_tag(&path.name, tag.as_str())
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
