//! OCI Blob Endpoints
//!
//! Handles both blob operations and upload start using a unified path structure.
//! The path `/v2/{name}/blobs/{ref}` handles:
//! - GET/HEAD/DELETE when `ref` is a digest (e.g., "sha256:abc...") - blob operations
//! - POST/PUT when `ref` is "uploads" - upload start operations
//!
//! This unified structure avoids Dropshot router conflicts between literal
//! and variable path segments while maintaining OCI spec compliance.
//!
//! # Streaming Limitations
//!
//! Dropshot does not currently support streaming request bodies, so upload operations
//! (monolithic PUT to `/v2/{name}/blobs/uploads?digest=...`) buffer the entire request
//! body into memory via `UntypedBody`. This limits practical upload sizes to available
//! server memory.
//!
//! For large blobs, clients should use chunked uploads instead:
//!
//! 1. `POST /v2/{name}/blobs/uploads` - Start an upload session
//! 2. `PATCH /v2/{name}/blobs/uploads/{session_id}` - Upload chunks (repeated)
//! 3. `PUT /v2/{name}/blobs/uploads/{session_id}?digest=...` - Complete the upload
//!
//! See the `uploads` module for chunked upload endpoints.

use bencher_endpoint::{CorsResponse, Delete, Endpoint, Get, Post, Put};
use bencher_json::ProjectResourceId;
use bencher_oci_storage::{Digest, OciError};
use bencher_schema::context::ApiContext;
use dropshot::{
    Body, ClientErrorStatusCode, HttpError, Path, Query, RequestContext, UntypedBody, endpoint,
};
use http::Response;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::auth::{
    require_pull_access, require_push_access, resolve_project_uuid, validate_push_access,
};
use crate::response::{
    APPLICATION_OCTET_STREAM, DOCKER_CONTENT_DIGEST, DOCKER_UPLOAD_UUID, oci_cors_headers,
};

/// Path parameters for blob/upload-start endpoints
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BlobPath {
    /// Project resource ID (UUID or slug)
    pub name: ProjectResourceId,
    /// Reference - either a digest (e.g., "sha256:abc123...") or "uploads"
    #[serde(rename = "ref")]
    pub reference: String,
}

/// Query parameters for upload start (optional mount)
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadStartQuery {
    /// Digest of blob to mount from another repository
    pub digest: Option<String>,
    /// Source repository for cross-repo mount
    pub from: Option<String>,
}

/// Query parameters for monolithic upload
#[derive(Debug, Deserialize, JsonSchema)]
pub struct MonolithicUploadQuery {
    /// Expected digest of the complete blob
    pub digest: String,
}

/// CORS preflight for blob/upload endpoints
#[endpoint {
    method = OPTIONS,
    path = "/v2/{name}/blobs/{ref}",
    tags = ["oci"],
    unpublished = true,
}]
pub async fn oci_blob_options(
    _rqctx: RequestContext<ApiContext>,
    _path: Path<BlobPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[
        Get.into(),
        Delete.into(),
        Post.into(),
        Put.into(),
    ]))
}

/// Check if a blob exists (HEAD) or start upload check
#[endpoint {
    method = HEAD,
    path = "/v2/{name}/blobs/{ref}",
    tags = ["oci"],
}]
pub async fn oci_blob_exists(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();

    // "uploads" is not a valid digest - return appropriate error
    if path.reference == "uploads" {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::METHOD_NOT_ALLOWED,
            "HEAD not supported for uploads endpoint".to_owned(),
        ));
    }

    // Authenticate and apply rate limiting
    let name_str = path.name.to_string();
    let _access = require_pull_access(&rqctx, &name_str).await?;

    // Resolve project UUID for stable storage paths
    let project_uuid = resolve_project_uuid(context, &path.name).await?;

    // Parse digest
    let digest: Digest = crate::error::parse_digest(&path.reference)?;

    // Get storage
    let storage = context.oci_storage();

    // Check if blob exists and get size
    let size = storage
        .get_blob_size(&project_uuid, &digest)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Build response with OCI-compliant headers (no body for HEAD)
    let response = oci_cors_headers(
        Response::builder()
            .status(http::StatusCode::OK)
            .header(http::header::CONTENT_TYPE, APPLICATION_OCTET_STREAM)
            .header(http::header::CONTENT_LENGTH, size)
            .header(DOCKER_CONTENT_DIGEST, digest.to_string())
            .header(http::header::ACCEPT_RANGES, "bytes"),
        &[http::Method::HEAD, http::Method::GET],
    )
    .body(Body::empty())
    .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Download a blob (GET)
///
/// Streams the blob content rather than loading it entirely into memory,
/// making this suitable for large container image layers.
#[endpoint {
    method = GET,
    path = "/v2/{name}/blobs/{ref}",
    tags = ["oci"],
}]
pub async fn oci_blob_get(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();

    // "uploads" is not a valid digest - return appropriate error
    if path.reference == "uploads" {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::METHOD_NOT_ALLOWED,
            "GET not supported for uploads endpoint without session ID".to_owned(),
        ));
    }

    // Authenticate and apply rate limiting
    let name_str = path.name.to_string();
    let _access = require_pull_access(&rqctx, &name_str).await?;

    // Resolve project UUID for stable storage paths
    let project_uuid = resolve_project_uuid(context, &path.name).await?;

    // Parse digest
    let digest: Digest = crate::error::parse_digest(&path.reference)?;

    // Get storage
    let storage = context.oci_storage();

    // Get blob as streaming body
    let (blob_body, size) = storage
        .get_blob_stream(&project_uuid, &digest)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciBlobPull);

    // Build response with OCI-compliant headers and streaming body
    let response = oci_cors_headers(
        Response::builder()
            .status(http::StatusCode::OK)
            .header(http::header::CONTENT_TYPE, APPLICATION_OCTET_STREAM)
            .header(http::header::CONTENT_LENGTH, size)
            .header(DOCKER_CONTENT_DIGEST, digest.to_string())
            .header(http::header::ACCEPT_RANGES, "bytes"),
        &[http::Method::GET],
    )
    .body(Body::wrap(blob_body))
    .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Delete a blob (DELETE)
#[endpoint {
    method = DELETE,
    path = "/v2/{name}/blobs/{ref}",
    tags = ["oci"],
}]
pub async fn oci_blob_delete(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();

    // "uploads" is not a valid digest - return appropriate error
    if path.reference == "uploads" {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::METHOD_NOT_ALLOWED,
            "DELETE not supported for uploads endpoint without session ID".to_owned(),
        ));
    }

    // Authenticate and apply rate limiting (delete requires push permission)
    let name_str = path.name.to_string();
    let _access = require_push_access(&rqctx, &name_str).await?;

    // Resolve project UUID for stable storage paths
    let project_uuid = resolve_project_uuid(context, &path.name).await?;

    // Parse digest
    let digest: Digest = crate::error::parse_digest(&path.reference)?;

    // Get storage
    let storage = context.oci_storage();

    // Verify blob exists before deleting (S3 silently succeeds for missing objects)
    let exists = storage
        .blob_exists(&project_uuid, &digest)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;
    if !exists {
        return Err(crate::error::into_http_error(OciError::BlobUnknown {
            digest: digest.to_string(),
        }));
    }

    // Delete the blob
    storage
        .delete_blob(&project_uuid, &digest)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // OCI spec requires 202 Accepted for DELETE
    let response = oci_cors_headers(
        Response::builder().status(http::StatusCode::ACCEPTED),
        &[http::Method::DELETE],
    )
    .body(Body::empty())
    .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Start a new blob upload (POST to /v2/{name}/blobs/uploads)
///
/// Authentication is optional for unclaimed projects.
/// If the project's organization is claimed, valid authentication with Create permission is required.
/// If the project doesn't exist and a slug is used, the project will be created automatically.
#[endpoint {
    method = POST,
    path = "/v2/{name}/blobs/{ref}",
    tags = ["oci"],
}]
pub async fn oci_upload_start(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
    query: Query<UploadStartQuery>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();
    let query = query.into_inner();

    // POST is only valid when ref is "uploads"
    if path.reference != "uploads" {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::METHOD_NOT_ALLOWED,
            "POST only supported for uploads endpoint".to_owned(),
        ));
    }

    // Validate push access and get or create the project
    let push_access = validate_push_access(&rqctx.log, &rqctx, &path.name).await?;
    let project_slug = &push_access.project.slug;
    let project_uuid = push_access.project.uuid;

    // Get storage
    let storage = context.oci_storage();

    // Handle cross-repository mount if requested
    if let (Some(digest_str), Some(from_name)) = (&query.digest, &query.from) {
        let digest: Digest = crate::error::parse_digest(digest_str)?;
        let from_repo: ProjectResourceId = from_name.parse().map_err(|_err| {
            crate::error::into_http_error(OciError::NameInvalid {
                name: from_name.clone(),
            })
        })?;

        // Only attempt mount if caller has pull access to source repository.
        // If pull access is denied, fall through to normal upload
        // to avoid revealing whether the source repository exists.
        let from_repo_str = from_repo.to_string();
        let pull_result = require_pull_access(&rqctx, &from_repo_str).await;
        if pull_result.is_ok() {
            // Resolve source repository UUID
            let from_uuid = resolve_project_uuid(context, &from_repo).await?;
            // Try to mount the blob
            let mounted = storage
                .mount_blob(&from_uuid, &project_uuid, &digest)
                .await
                .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

            if mounted {
                // Mount successful - return 201 Created
                let location = format!("/v2/{project_slug}/blobs/{digest}");
                let response = oci_cors_headers(
                    Response::builder()
                        .status(http::StatusCode::CREATED)
                        .header(http::header::LOCATION, location)
                        .header(DOCKER_CONTENT_DIGEST, digest.to_string()),
                    &[http::Method::POST],
                )
                .body(Body::empty())
                .map_err(|e| {
                    HttpError::for_internal_error(format!("Failed to build response: {e}"))
                })?;
                return Ok(response);
            }
        } else {
            slog::info!(rqctx.log, "Cross-repository mount access denied, falling through to upload";
                "from_repo" => &from_repo_str, "to_repo" => %path.name, "digest" => %digest);
            #[cfg(feature = "sentry")]
            sentry::capture_message(
                &format!(
                    "OCI cross-repo mount denied: from={from_repo_str} to={} digest={digest}",
                    path.name
                ),
                sentry::Level::Warning,
            );
        }
    }

    // Start a new upload session
    let upload_id = storage
        .start_upload(&project_uuid)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Build 202 Accepted response
    let location = format!("/v2/{project_slug}/blobs/uploads/{upload_id}");
    let response = oci_cors_headers(
        Response::builder()
            .status(http::StatusCode::ACCEPTED)
            .header(http::header::LOCATION, location)
            .header(http::header::RANGE, "0-0")
            .header(DOCKER_UPLOAD_UUID, upload_id.to_string()),
        &[http::Method::POST],
    )
    .body(Body::empty())
    .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Monolithic upload (PUT to /v2/{name}/blobs/uploads?digest=...)
///
/// Uploads a complete blob in a single request. This is convenient for small blobs but
/// has memory limitations: Dropshot's `UntypedBody` buffers the entire request into memory.
/// For large blobs, use the chunked upload flow instead (see `uploads` module).
///
/// # Memory Limitation
///
/// The entire blob content is buffered in server memory before being stored. This means
/// practical upload sizes are limited by available server memory. For blobs larger than
/// a few MB, consider using chunked uploads:
///
/// 1. `POST /v2/{name}/blobs/uploads` - Start session
/// 2. `PATCH /v2/{name}/blobs/uploads/{session_id}` - Upload chunks
/// 3. `PUT /v2/{name}/blobs/uploads/{session_id}?digest=...` - Complete
///
/// # Authentication
///
/// Authentication is optional for unclaimed projects.
/// If the project's organization is claimed, valid authentication with Create permission is required.
/// If the project doesn't exist and a slug is used, the project will be created automatically.
#[endpoint {
    method = PUT,
    path = "/v2/{name}/blobs/{ref}",
    tags = ["oci"],
}]
pub async fn oci_upload_monolithic(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
    query: Query<MonolithicUploadQuery>,
    body: UntypedBody,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();
    let query = query.into_inner();
    let data = body.as_bytes();

    // PUT with digest query param is only valid when ref is "uploads"
    if path.reference != "uploads" {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::METHOD_NOT_ALLOWED,
            "PUT only supported for uploads endpoint".to_owned(),
        ));
    }

    // Validate push access and get or create the project
    let push_access = validate_push_access(&rqctx.log, &rqctx, &path.name).await?;
    let project_slug = &push_access.project.slug;
    let project_uuid = push_access.project.uuid;

    // Get storage and enforce max body size
    let storage = context.oci_storage();
    let max = storage.max_body_size();
    if data.len() as u64 > max {
        return Err(crate::error::payload_too_large(data.len() as u64, max));
    }

    // Parse digest
    let expected_digest: Digest = crate::error::parse_digest(&query.digest)?;

    // Start upload, append data, and complete in one operation
    let upload_id = storage
        .start_upload(&project_uuid)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Copy is unavoidable: Dropshot's UntypedBody only provides as_bytes() -> &[u8]
    let result = async {
        storage
            .append_upload(&upload_id, bytes::Bytes::copy_from_slice(data))
            .await?;
        storage.complete_upload(&upload_id, &expected_digest).await
    }
    .await;

    let actual_digest = match result {
        Ok(digest) => digest,
        Err(e) => {
            // Best-effort cleanup of the orphaned upload session
            let _unused = storage.cancel_upload(&upload_id).await;
            return Err(crate::error::into_http_error(OciError::from(e)));
        },
    };

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciBlobPush);

    // Build 201 Created response
    let location = format!("/v2/{project_slug}/blobs/{actual_digest}");
    let response = oci_cors_headers(
        Response::builder()
            .status(http::StatusCode::CREATED)
            .header(http::header::LOCATION, location)
            .header(DOCKER_CONTENT_DIGEST, actual_digest.to_string()),
        &[http::Method::PUT],
    )
    .body(Body::empty())
    .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}
