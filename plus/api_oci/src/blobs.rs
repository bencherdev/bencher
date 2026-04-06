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
//! # Streaming Uploads
//!
//! All blob upload endpoints use `StreamingBody` to stream request data directly
//! to storage without buffering the entire body in memory. Per-endpoint
//! `request_body_max_bytes` is set to 10 GiB; the cumulative size limit is
//! enforced by `OciStorage::max_body_size()` at the storage layer.
//!
//! For large blobs, clients can also use chunked uploads:
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
    Body, ClientErrorStatusCode, HttpError, Path, Query, RequestContext, StreamingBody, endpoint,
};
use http::Response;
use schemars::JsonSchema;
use serde::Deserialize;

/// Path segment distinguishing upload endpoints from blob digest endpoints.
pub const UPLOADS_REF: &str = "uploads";

use crate::auth::{
    check_oci_bandwidth, record_oci_bandwidth, require_push_access, resolve_project,
    validate_pull_access, validate_push_access,
};
use crate::error::storage_error;
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
    if path.reference == UPLOADS_REF {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::METHOD_NOT_ALLOWED,
            "HEAD not supported for uploads endpoint".to_owned(),
        ));
    }

    // Authenticate (optional for unclaimed projects) and resolve project
    let project = validate_pull_access(&rqctx, &path.name).await?;
    let project_uuid = project.uuid;

    // Parse digest
    let digest: Digest = crate::error::parse_digest(&path.reference)?;

    // Get storage
    let storage = context.oci_storage();

    // Check if blob exists and get size
    let size = storage
        .get_blob_size(&project_uuid, &digest)
        .await
        .map_err(storage_error)?;

    // Build response with OCI-compliant headers (no body for HEAD)
    let response = oci_cors_headers(
        Response::builder()
            .status(http::StatusCode::OK)
            .header(http::header::CONTENT_TYPE, APPLICATION_OCTET_STREAM)
            .header(http::header::CONTENT_LENGTH, size)
            .header(DOCKER_CONTENT_DIGEST, digest.to_string()),
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
    if path.reference == UPLOADS_REF {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::METHOD_NOT_ALLOWED,
            "GET not supported for uploads endpoint without session ID".to_owned(),
        ));
    }

    // Authenticate (public tokens restricted to unclaimed projects) and resolve project
    let project = validate_pull_access(&rqctx, &path.name).await?;
    let project_uuid = project.uuid;

    // Check bandwidth limit before transfer
    let org_id = check_oci_bandwidth(context, &project).await?;

    // Parse digest
    let digest: Digest = crate::error::parse_digest(&path.reference)?;

    // Get storage
    let storage = context.oci_storage();

    // Get blob as streaming body
    let (blob_body, size) = storage
        .get_blob_stream(&project_uuid, &digest)
        .await
        .map_err(storage_error)?;

    // Record bandwidth usage
    record_oci_bandwidth(context, org_id, size);

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciBlobPull);

    // Build response with OCI-compliant headers and streaming body
    let response = oci_cors_headers(
        Response::builder()
            .status(http::StatusCode::OK)
            .header(http::header::CONTENT_TYPE, APPLICATION_OCTET_STREAM)
            .header(http::header::CONTENT_LENGTH, size)
            .header(DOCKER_CONTENT_DIGEST, digest.to_string()),
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
    if path.reference == UPLOADS_REF {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::METHOD_NOT_ALLOWED,
            "DELETE not supported for uploads endpoint without session ID".to_owned(),
        ));
    }

    // Authenticate and apply rate limiting (delete requires push permission)
    let name_str = path.name.to_string();
    require_push_access(&rqctx, &name_str).await?;

    // Resolve project for stable storage paths
    let project = resolve_project(context, &path.name).await?;
    let project_uuid = project.uuid;

    // Parse digest
    let digest: Digest = crate::error::parse_digest(&path.reference)?;

    // Get storage
    let storage = context.oci_storage();

    // Verify blob exists before deleting (S3 silently succeeds for missing objects)
    let exists = storage
        .blob_exists(&project_uuid, &digest)
        .await
        .map_err(storage_error)?;
    if !exists {
        return Err(crate::error::into_http_error(OciError::BlobUnknown {
            digest: digest.to_string(),
        }));
    }

    // Delete the blob
    storage
        .delete_blob(&project_uuid, &digest)
        .await
        .map_err(storage_error)?;

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
    if path.reference != UPLOADS_REF {
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

    // Check bandwidth limit before any potential data transfer (mount)
    let org_id = check_oci_bandwidth(context, &push_access.project).await?;

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
        if let Ok(from_project) = validate_pull_access(&rqctx, &from_repo).await {
            // Try to mount the blob — fall through on failure
            if let Ok(true) = storage
                .mount_blob(&from_project.uuid, &project_uuid, &digest)
                .await
            {
                // Record bandwidth for the mounted blob
                if let Ok(size) = storage.get_blob_size(&project_uuid, &digest).await {
                    record_oci_bandwidth(context, org_id, size);
                }

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
                "from_repo" => %from_repo, "to_repo" => %path.name, "digest" => %digest);
            #[cfg(feature = "sentry")]
            sentry::capture_message(
                &format!(
                    "OCI cross-repo mount denied: from={from_repo} to={} digest={digest}",
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
        .map_err(storage_error)?;

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
/// Uploads a complete blob in a single request. The request body is streamed
/// to storage incrementally, so memory usage is bounded by the network chunk size
/// rather than the total blob size.
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
    request_body_max_bytes = crate::OCI_REQUEST_BODY_MAX_BYTES,
}]
pub async fn oci_upload_monolithic(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
    query: Query<MonolithicUploadQuery>,
    body: StreamingBody,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();
    let query = query.into_inner();

    // PUT with digest query param is only valid when ref is "uploads"
    if path.reference != UPLOADS_REF {
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

    // Check bandwidth limit before transfer
    let org_id = check_oci_bandwidth(context, &push_access.project).await?;

    // Get storage
    let storage = context.oci_storage();

    // Parse digest
    let expected_digest: Digest = crate::error::parse_digest(&query.digest)?;

    // Start upload, stream data, and complete in one operation
    let upload_id = storage
        .start_upload(&project_uuid)
        .await
        .map_err(storage_error)?;

    // Stream body to storage (storage enforces max_body_size incrementally)
    let result = async {
        let final_size = crate::uploads::stream_to_storage(body, storage, &upload_id, 0).await?;
        let digest = storage
            .complete_upload(&upload_id, &expected_digest)
            .await
            .map_err(storage_error)?;
        Ok::<(Digest, u64), HttpError>((digest, final_size))
    }
    .await;

    let (actual_digest, final_size) = match result {
        Ok(result) => result,
        Err(e) => {
            // Best-effort cleanup of the orphaned upload session
            if let Err(cancel_err) = storage.cancel_upload(&upload_id).await {
                slog::info!(rqctx.log, "Failed to cancel upload session on error"; "upload_id" => %upload_id, "error" => %cancel_err);
            }
            return Err(e);
        },
    };

    // Record bandwidth usage
    record_oci_bandwidth(context, org_id, final_size);

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
