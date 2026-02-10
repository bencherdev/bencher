//! OCI Manifest Endpoints
//!
//! - HEAD /v2/<name>/manifests/<reference> - Check manifest existence
//! - GET /v2/<name>/manifests/<reference> - Download manifest
//! - PUT /v2/<name>/manifests/<reference> - Upload manifest
//! - DELETE /v2/<name>/manifests/<reference> - Delete manifest

use bencher_endpoint::{CorsResponse, Delete, Endpoint, Get, Put};
use bencher_json::ProjectResourceId;
use bencher_json::oci::Manifest;
use bencher_oci_storage::{Digest, MAX_CONCURRENCY, OciError, OciStorage, ProjectUuid, Reference};
use bencher_schema::context::ApiContext;
use dropshot::{Body, HttpError, Path, RequestContext, UntypedBody, endpoint};
use futures::stream::{self, TryStreamExt as _};
use http::Response;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::auth::{
    require_pull_access, require_push_access, resolve_project_uuid, validate_push_access,
};
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

/// Parse a reference for pull (HEAD/GET) operations.
///
/// For read operations, an unparseable reference simply means the manifest doesn't
/// exist — return 404 `MANIFEST_UNKNOWN` instead of 400 `TAG_INVALID`/`DIGEST_INVALID`.
/// This matches OCI Distribution Spec conformance expectations.
fn parse_reference_for_pull(reference: &str) -> Result<Reference, HttpError> {
    reference.parse().map_err(|_err| {
        crate::error::into_http_error(OciError::ManifestUnknown {
            reference: reference.to_owned(),
        })
    })
}

/// Resolve a reference (tag or digest) to a digest
async fn resolve_reference(
    storage: &OciStorage,
    name: &ProjectUuid,
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

    // Resolve project UUID for stable storage paths
    let project_uuid = resolve_project_uuid(context, &path.name).await?;

    // Parse reference (pull operation: unparseable → 404 MANIFEST_UNKNOWN)
    let reference = parse_reference_for_pull(&path.reference)?;

    // Get storage
    let storage = context.oci_storage();

    // Resolve the reference to a digest
    let digest = resolve_reference(storage, &project_uuid, &reference).await?;

    // Get manifest to check existence and get size
    let manifest = storage
        .get_manifest_by_digest(&project_uuid, &digest)
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

    // Resolve project UUID for stable storage paths
    let project_uuid = resolve_project_uuid(context, &path.name).await?;

    // Parse reference (pull operation: unparseable → 404 MANIFEST_UNKNOWN)
    let reference = parse_reference_for_pull(&path.reference)?;

    // Get storage
    let storage = context.oci_storage();

    // Resolve the reference to a digest
    let digest = resolve_reference(storage, &project_uuid, &reference).await?;

    // Get manifest content
    let manifest = storage
        .get_manifest_by_digest(&project_uuid, &digest)
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
    let project_uuid = push_access.project.uuid;

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
        let ct_base = ct_str.split(';').next().unwrap_or(ct_str).trim();
        if ct_base != manifest_media_type {
            return Err(crate::error::into_http_error(OciError::ManifestInvalid(
                format!(
                    "Content-Type '{ct_str}' does not match manifest mediaType '{manifest_media_type}'"
                ),
            )));
        }
    }

    // Verify referenced blobs exist (for image manifests and Docker manifests)
    verify_referenced_blobs(storage, &project_uuid, &parsed_manifest).await?;

    // Extract subject digest from manifest if present (for OCI-Subject header)
    let subject_digest = parsed_manifest.subject().map(|s| s.digest.clone());

    // Pre-compute digest from body bytes to verify against URL digest BEFORE storing
    let content_digest = Digest::from_sha256_bytes(body_bytes);
    if let Reference::Digest(ref_digest) = &reference
        && ref_digest != &content_digest
    {
        return Err(crate::error::into_http_error(OciError::DigestInvalid {
            digest: format!(
                "URL digest {ref_digest} does not match content digest {content_digest}"
            ),
        }));
    }

    // Store the manifest, passing the already-parsed manifest to avoid re-parsing
    // Copy is unavoidable: Dropshot's UntypedBody only provides as_bytes() -> &[u8]
    let digest = storage
        .put_manifest(
            &project_uuid,
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

/// Verify that all blobs referenced by a manifest exist in storage
///
/// Only checks OCI Image Manifests and Docker Manifest V2 (which have config/layers).
/// Image indices and manifest lists reference other manifests, not blobs.
/// Checks all blobs in parallel for improved performance.
async fn verify_referenced_blobs(
    storage: &OciStorage,
    repository: &ProjectUuid,
    manifest: &Manifest,
) -> Result<(), HttpError> {
    let digests: Vec<&str> = match manifest {
        Manifest::OciImageManifest(m) => {
            let mut d = vec![m.config.digest.as_str()];
            d.extend(m.layers.iter().map(|l| l.digest.as_str()));
            d
        },
        Manifest::DockerManifestV2(m) => {
            let mut d = vec![m.config.digest.as_str()];
            d.extend(m.layers.iter().map(|l| l.digest.as_str()));
            d
        },
        // Image indices/manifest lists reference manifests, not blobs
        Manifest::OciImageIndex(_) | Manifest::DockerManifestList(_) => return Ok(()),
    };

    // Parse all digests first, then check existence in parallel
    let parsed_digests: Vec<(Digest, String)> = digests
        .iter()
        .map(|d| {
            d.parse::<Digest>()
                .map(|parsed| (parsed, (*d).to_owned()))
                .map_err(|_e| {
                    crate::error::into_http_error(OciError::DigestInvalid {
                        digest: (*d).to_owned(),
                    })
                })
        })
        .collect::<Result<_, _>>()?;

    let concurrency = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
        .clamp(1, MAX_CONCURRENCY);

    stream::iter(parsed_digests.into_iter().map(Ok))
        .try_for_each_concurrent(concurrency, |(digest, digest_str)| async move {
            let exists = storage
                .blob_exists(repository, &digest)
                .await
                .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;
            if !exists {
                return Err(crate::error::into_http_error(
                    OciError::ManifestBlobUnknown { digest: digest_str },
                ));
            }
            Ok(())
        })
        .await?;

    Ok(())
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

    // Resolve project UUID for stable storage paths
    let project_uuid = resolve_project_uuid(context, &path.name).await?;

    // Get storage
    let storage = context.oci_storage();

    // Parse reference - can be either a digest or a tag
    let reference = parse_reference(&path.reference)?;

    // Resolve the digest for the response header before deleting
    let digest = match &reference {
        Reference::Digest(digest) => digest.clone(),
        Reference::Tag(tag) => storage
            .resolve_tag(&project_uuid, tag)
            .await
            .map_err(|e| crate::error::into_http_error(OciError::from(e)))?,
    };

    match &reference {
        Reference::Digest(digest) => {
            // Delete by digest - delete the manifest itself
            storage
                .delete_manifest(&project_uuid, digest)
                .await
                .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;
        },
        Reference::Tag(tag) => {
            // Delete by tag - delete the tag link only (manifest may still exist)
            storage
                .delete_tag(&project_uuid, tag)
                .await
                .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;
        },
    }

    // OCI spec requires 202 Accepted for DELETE
    let response = oci_cors_headers(
        Response::builder()
            .status(http::StatusCode::ACCEPTED)
            .header(DOCKER_CONTENT_DIGEST, digest.to_string()),
        &[http::Method::DELETE],
    )
    .body(Body::empty())
    .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}
