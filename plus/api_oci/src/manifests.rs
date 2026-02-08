//! OCI Manifest Endpoints
//!
//! - HEAD /v2/<name>/manifests/<reference> - Check manifest existence
//! - GET /v2/<name>/manifests/<reference> - Download manifest
//! - PUT /v2/<name>/manifests/<reference> - Upload manifest
//! - DELETE /v2/<name>/manifests/<reference> - Delete manifest

use bencher_endpoint::{CorsResponse, Delete, Endpoint, Get, Put};
use bencher_json::ProjectResourceId;
use bencher_json::oci::Manifest;
use bencher_oci_storage::{Digest, OciError, OciStorage, Reference};
use bencher_schema::context::ApiContext;
use dropshot::{Body, HttpError, Path, RequestContext, UntypedBody, endpoint};
use http::Response;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::auth::{require_pull_access, require_push_access, validate_push_access};
use crate::response::{DOCKER_CONTENT_DIGEST, OCI_SUBJECT, oci_cors_headers};

/// Parse a reference string, returning the correct OCI error code on failure.
///
/// Per the OCI Distribution Spec, an invalid digest should return `DIGEST_INVALID`
/// and an invalid tag should return `TAG_INVALID` — not `MANIFEST_UNKNOWN` (which
/// is a 404 for a well-formed reference that simply doesn't exist).
fn parse_reference(reference: &str) -> Result<Reference, HttpError> {
    reference.parse().map_err(|_err| {
        if reference.contains(':') {
            crate::error::into_http_error(OciError::DigestInvalid {
                digest: reference.to_owned(),
            })
        } else {
            crate::error::into_http_error(OciError::TagInvalid {
                tag: reference.to_owned(),
            })
        }
    })
}

/// Resolve a reference (tag or digest) to a digest
async fn resolve_reference(
    storage: &OciStorage,
    name: &ProjectResourceId,
    reference: &Reference,
) -> Result<Digest, HttpError> {
    match reference {
        Reference::Digest(d) => Ok(d.clone()),
        Reference::Tag(t) => storage
            .resolve_tag(name, t)
            .await
            .map_err(|e| crate::error::into_http_error(OciError::from(e))),
    }
}

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
    unpublished = true,
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
    let context = rqctx.context();
    let path = path.into_inner();

    // Authenticate and apply rate limiting
    let name_str = path.name.to_string();
    let _access = require_pull_access(&rqctx, &name_str).await?;

    // Parse reference
    let reference = parse_reference(&path.reference)?;

    // Get storage
    let storage = context.oci_storage();

    // Resolve the reference to a digest
    let digest = resolve_reference(storage, &path.name, &reference).await?;

    // Get manifest to check existence and get size
    let manifest = storage
        .get_manifest_by_digest(&path.name, &digest)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Determine content type from typed manifest
    let parsed = Manifest::from_bytes(&manifest).map_err(|e| {
        HttpError::for_internal_error(format!("Failed to parse stored manifest: {e}"))
    })?;
    let content_type = parsed.media_type().to_owned();

    // Build response with OCI-compliant headers (no body for HEAD)
    let response = oci_cors_headers(
        Response::builder()
            .status(http::StatusCode::OK)
            .header(http::header::CONTENT_TYPE, content_type)
            .header(http::header::CONTENT_LENGTH, manifest.len())
            .header(DOCKER_CONTENT_DIGEST, digest.to_string()),
        &[http::Method::HEAD, http::Method::GET],
    )
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
    let context = rqctx.context();
    let path = path.into_inner();

    // Authenticate and apply rate limiting
    let name_str = path.name.to_string();
    let _access = require_pull_access(&rqctx, &name_str).await?;

    // Parse reference
    let reference = parse_reference(&path.reference)?;

    // Get storage
    let storage = context.oci_storage();

    // Resolve the reference to a digest
    let digest = resolve_reference(storage, &path.name, &reference).await?;

    // Get manifest content
    let manifest = storage
        .get_manifest_by_digest(&path.name, &digest)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciManifestPull);

    // Determine content type from typed manifest
    let parsed = Manifest::from_bytes(&manifest).map_err(|e| {
        HttpError::for_internal_error(format!("Failed to parse stored manifest: {e}"))
    })?;
    let content_type = parsed.media_type().to_owned();

    // Build response with OCI-compliant headers
    let response = oci_cors_headers(
        Response::builder()
            .status(http::StatusCode::OK)
            .header(http::header::CONTENT_TYPE, content_type)
            .header(http::header::CONTENT_LENGTH, manifest.len())
            .header(DOCKER_CONTENT_DIGEST, digest.to_string()),
        &[http::Method::GET],
    )
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
    let reference = parse_reference(&path.reference)?;

    // Get storage and enforce max body size
    let storage = context.oci_storage();
    let max = storage.max_body_size();
    if body_bytes.len() as u64 > max {
        return Err(crate::error::payload_too_large(
            body_bytes.len() as u64,
            max,
        ));
    }

    // Determine tag from reference (if it's a tag)
    let tag = match &reference {
        Reference::Tag(t) => Some(t),
        Reference::Digest(_) => None,
    };

    // Parse and validate manifest using typed schemas
    let parsed_manifest = Manifest::from_bytes(body_bytes).map_err(|e| {
        crate::error::into_http_error(OciError::ManifestInvalid(format!("Invalid manifest: {e}")))
    })?;

    // Validate Content-Type header matches manifest mediaType if present
    if let Some(content_type) = rqctx.request.headers().get(http::header::CONTENT_TYPE)
        && let Ok(ct_str) = content_type.to_str()
    {
        let manifest_media_type = parsed_manifest.media_type();
        if ct_str != manifest_media_type {
            return Err(crate::error::into_http_error(OciError::ManifestInvalid(
                format!(
                    "Content-Type '{ct_str}' does not match manifest mediaType '{manifest_media_type}'"
                ),
            )));
        }
    }

    // Extract subject digest from manifest if present (for OCI-Subject header)
    let subject_digest = parsed_manifest.subject().map(|s| s.digest.clone());

    // Store the manifest, passing the already-parsed manifest to avoid re-parsing
    // Copy is unavoidable: Dropshot's UntypedBody only provides as_bytes() -> &[u8]
    let digest = storage
        .put_manifest(
            &path.name,
            bytes::Bytes::copy_from_slice(body_bytes),
            tag,
            &parsed_manifest,
        )
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciManifestPush);

    // Build 201 Created response with Location and Docker-Content-Digest headers
    let location = format!("/v2/{project_slug}/manifests/{digest}");

    let mut builder = oci_cors_headers(
        Response::builder()
            .status(http::StatusCode::CREATED)
            .header(http::header::LOCATION, location)
            .header(DOCKER_CONTENT_DIGEST, digest.to_string()),
        &[http::Method::PUT],
    );

    // Add OCI-Subject header if manifest has a subject field
    if let Some(subject) = subject_digest {
        builder = builder.header(OCI_SUBJECT, subject);
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
    let context = rqctx.context();
    let path = path.into_inner();

    // Authenticate and apply rate limiting (delete requires push permission)
    let name_str = path.name.to_string();
    let _access = require_push_access(&rqctx, &name_str).await?;

    // Get storage
    let storage = context.oci_storage();

    // Parse reference - can be either a digest or a tag
    let reference = parse_reference(&path.reference)?;

    match reference {
        Reference::Digest(digest) => {
            // Delete by digest - delete the manifest itself
            storage
                .delete_manifest(&path.name, &digest)
                .await
                .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;
        },
        Reference::Tag(tag) => {
            // Delete by tag - delete the tag link only (manifest may still exist)
            storage
                .delete_tag(&path.name, &tag)
                .await
                .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;
        },
    }

    // OCI spec requires 202 Accepted for DELETE
    let response = oci_cors_headers(
        Response::builder().status(http::StatusCode::ACCEPTED),
        &[http::Method::DELETE],
    )
    .body(Body::empty())
    .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}
