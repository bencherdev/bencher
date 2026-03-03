//! OCI Upload Session Endpoints
//!
//! Handles upload session operations at `/v2/{name}/blobs/{ref}/{session_id}`.
//! The `ref` parameter must be "uploads" for these endpoints.
//!
//! - GET - Get upload status
//! - PATCH - Upload chunk
//! - PUT - Complete upload
//! - DELETE - Cancel upload
//!
//! # Chunked Upload Flow
//!
//! For large blobs that would exceed server memory limits with monolithic uploads,
//! use the chunked upload flow:
//!
//! 1. Start a session: `POST /v2/{name}/blobs/uploads` (requires authentication)
//! 2. Upload chunks: `PATCH /v2/{name}/blobs/uploads/{session_id}` (repeated)
//! 3. Complete: `PUT /v2/{name}/blobs/uploads/{session_id}?digest=sha256:...`
//!
//! Each chunk is stored on upload, so memory usage is bounded by the chunk size
//! rather than the total blob size. Clients can use any chunk size they prefer.
//!
//! # Authentication
//!
//! These endpoints do NOT require Bearer token authentication.
//! The session ID itself serves as authentication - it can only be obtained
//! by authenticating to POST /v2/{name}/blobs/uploads/, and session IDs
//! are unguessable UUIDs. This matches OCI spec behavior and is required
//! for conformance test compatibility.

#[cfg(feature = "plus")]
use crate::auth::apply_public_rate_limit;
use crate::auth::resolve_project_uuid;
use crate::blobs::UPLOADS_REF;
use crate::response::{DOCKER_CONTENT_DIGEST, DOCKER_UPLOAD_UUID, oci_cors_headers};
use bencher_endpoint::{CorsResponse, Delete, Endpoint, Get, Patch, Put};
use bencher_json::ProjectResourceId;
use bencher_oci_storage::{Digest, OciError, UploadId};
use bencher_schema::context::ApiContext;
use dropshot::{
    Body, ClientErrorStatusCode, HttpError, Path, Query, RequestContext, StreamingBody, endpoint,
};
use futures::StreamExt as _;
use http::Response;
use schemars::JsonSchema;
use serde::Deserialize;

/// Path parameters for upload session operations
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadSessionPath {
    /// Project resource ID (UUID or slug)
    pub name: ProjectResourceId,
    /// Must be "uploads" for upload operations
    #[serde(rename = "ref")]
    pub reference: String,
    /// Upload session ID
    pub session_id: String,
}

/// Query parameters for upload completion
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadCompleteQuery {
    /// Expected digest of the complete blob
    pub digest: String,
}

/// Validate that the reference is "uploads"
fn validate_uploads_ref(reference: &str) -> Result<(), HttpError> {
    if reference != UPLOADS_REF {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::NOT_FOUND,
            format!("Invalid path: expected 'uploads', got '{reference}'"),
        ));
    }
    Ok(())
}

/// Format a Range header value for upload progress.
///
/// Per OCI spec, Range is `0-0` when empty, or `0-{size-1}` when data exists.
fn format_upload_range(size: u64) -> String {
    if size > 0 {
        format!("0-{}", size - 1)
    } else {
        "0-0".to_owned()
    }
}

/// Stream request body chunks directly to OCI storage.
///
/// Each network-level chunk is passed to `append_upload`, which stores it
/// incrementally and enforces `max_body_size`. Returns the total bytes received.
pub(crate) async fn stream_to_storage(
    body: StreamingBody,
    storage: &bencher_oci_storage::OciStorage,
    upload_id: &UploadId,
) -> Result<u64, HttpError> {
    let stream = body.into_stream();
    futures::pin_mut!(stream);
    let mut new_size = 0u64;
    while let Some(chunk) = stream.next().await {
        let data = chunk?;
        if data.is_empty() {
            continue;
        }
        new_size = storage
            .append_upload(upload_id, data)
            .await
            .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;
    }
    Ok(new_size)
}

/// CORS preflight for upload session operations
#[endpoint {
    method = OPTIONS,
    path = "/v2/{name}/blobs/{ref}/{session_id}",
    tags = ["oci"],
    unpublished = true,
}]
pub async fn oci_upload_session_options(
    _rqctx: RequestContext<ApiContext>,
    _path: Path<UploadSessionPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[
        Get.into(),
        Patch.into(),
        Put.into(),
        Delete.into(),
    ]))
}

/// Get upload status
#[endpoint {
    method = GET,
    path = "/v2/{name}/blobs/{ref}/{session_id}",
    tags = ["oci"],
}]
pub async fn oci_upload_status(
    rqctx: RequestContext<ApiContext>,
    path: Path<UploadSessionPath>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();
    validate_uploads_ref(&path.reference)?;

    // No Bearer auth required - session ID serves as authentication
    // (obtained only via authenticated POST to start upload)

    // Apply public rate limiting based on IP
    #[cfg(feature = "plus")]
    apply_public_rate_limit(&rqctx.log, context, &rqctx)?;

    let repository_name = path.name.to_string();

    // Resolve project UUID for stable storage paths
    let project_uuid = resolve_project_uuid(context, &path.name).await?;

    // Parse upload ID
    let upload_id: UploadId = crate::error::parse_upload_id(&path.session_id)?;

    // Get storage
    let storage = context.oci_storage();

    // Validate upload belongs to this repository
    storage
        .validate_upload_repository(&upload_id, &project_uuid)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Get current upload size
    let size = storage
        .get_upload_size(&upload_id)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Build 204 No Content response with Range header (OCI spec)
    let location = format!("/v2/{repository_name}/blobs/uploads/{upload_id}");

    let response = oci_cors_headers(
        Response::builder()
            .status(http::StatusCode::NO_CONTENT)
            .header(http::header::LOCATION, location)
            .header(http::header::RANGE, format_upload_range(size))
            .header(DOCKER_UPLOAD_UUID, upload_id.to_string()),
        &[http::Method::GET],
    )
    .body(Body::empty())
    .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Upload a chunk of data
///
/// Appends a chunk of data to an in-progress upload session. This is the recommended
/// approach for large blobs, as each chunk is stored immediately rather than buffering
/// the entire blob in memory.
///
/// Clients can call this endpoint multiple times to upload a blob in pieces, then
/// call `PUT /v2/{name}/blobs/uploads/{session_id}?digest=...` to complete the upload.
///
/// The `Content-Range` header is optional but recommended - if provided, the server
/// validates that the chunk starts at the expected offset.
#[endpoint {
    method = PATCH,
    path = "/v2/{name}/blobs/{ref}/{session_id}",
    tags = ["oci"],
    request_body_max_bytes = crate::OCI_REQUEST_BODY_MAX_BYTES,
}]
pub async fn oci_upload_chunk(
    rqctx: RequestContext<ApiContext>,
    path: Path<UploadSessionPath>,
    body: StreamingBody,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();
    validate_uploads_ref(&path.reference)?;

    // No Bearer auth required - session ID serves as authentication
    // (obtained only via authenticated POST to start upload)

    // Apply public rate limiting based on IP
    #[cfg(feature = "plus")]
    apply_public_rate_limit(&rqctx.log, context, &rqctx)?;

    let repository_name = path.name.to_string();

    // Resolve project UUID for stable storage paths
    let project_uuid = resolve_project_uuid(context, &path.name).await?;

    // Parse upload ID
    let upload_id: UploadId = crate::error::parse_upload_id(&path.session_id)?;

    // Get storage
    let storage = context.oci_storage();

    // Validate upload belongs to this repository
    storage
        .validate_upload_repository(&upload_id, &project_uuid)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Get current upload size for Content-Range validation
    let current_size = storage
        .get_upload_size(&upload_id)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Parse Content-Range header upfront (before streaming) if present
    // Content-Range format can be:
    // - Standard HTTP: "bytes start-end/total" or "bytes start-end/*"
    // - OCI variant: "start-end" (just the range numbers)
    let expected_len = if let Some(content_range) =
        rqctx.request.headers().get(http::header::CONTENT_RANGE)
        && let Ok(range_str) = content_range.to_str()
    {
        // Parse the range, handling both formats
        let range_part = range_str.strip_prefix("bytes ").unwrap_or(range_str);
        // Remove trailing /total if present
        let range_nums = range_part.split_once('/').map_or(range_part, |(r, _)| r);

        if let Some((start_str, end_str)) = range_nums.split_once('-') {
            let start_ok = start_str.parse::<u64>().ok();
            let end_ok = end_str.parse::<u64>().ok();

            // If exactly one side parses, the range is malformed
            let partial_parse = start_ok.is_some() != end_ok.is_some();

            // If neither side parses, the range is also malformed
            let neither_parsed = start_ok.is_none() && end_ok.is_none();

            // Validate start offset matches current upload size
            let start_mismatch = start_ok.is_some_and(|start| start != current_size);

            // Check for inverted range (end < start)
            let inverted_range = match (start_ok, end_ok) {
                (Some(start), Some(end)) => end < start,
                _ => false,
            };

            if partial_parse || neither_parsed || start_mismatch || inverted_range {
                // Return 416 with Location and Range headers per OCI spec
                let location = format!("/v2/{repository_name}/blobs/uploads/{upload_id}");
                let response = oci_cors_headers(
                    Response::builder()
                        .status(http::StatusCode::RANGE_NOT_SATISFIABLE)
                        .header(http::header::LOCATION, location)
                        .header(http::header::RANGE, format_upload_range(current_size))
                        .header(DOCKER_UPLOAD_UUID, upload_id.to_string()),
                    &[http::Method::PATCH],
                )
                .body(Body::empty())
                .map_err(|e| {
                    HttpError::for_internal_error(format!("Failed to build response: {e}"))
                })?;
                return Ok(response);
            }

            // Compute expected length for post-stream validation
            match (start_ok, end_ok) {
                (Some(start), Some(end)) => Some(end - start + 1),
                _ => None,
            }
        } else {
            None
        }
    } else {
        None
    };

    // Stream body to storage (storage enforces max_body_size incrementally)
    let new_size = stream_to_storage(body, storage, &upload_id).await?;

    // Post-stream validation: verify bytes received matches Content-Range expected length.
    // Note: Data has already been appended to the upload session at this point.
    // This is intentional — the Range header in the 416 response reflects the
    // actual upload state, allowing the client to resume from the correct offset.
    // This matches spec-compliant registries (Zot, olareg) per OCI distribution-spec.
    if let Some(expected) = expected_len {
        let bytes_received = new_size - current_size;
        if bytes_received != expected {
            // Data is in the upload session but length doesn't match Content-Range
            let location = format!("/v2/{repository_name}/blobs/uploads/{upload_id}");
            let response = oci_cors_headers(
                Response::builder()
                    .status(http::StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(http::header::LOCATION, location)
                    .header(http::header::RANGE, format_upload_range(new_size))
                    .header(DOCKER_UPLOAD_UUID, upload_id.to_string()),
                &[http::Method::PATCH],
            )
            .body(Body::empty())
            .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;
            return Ok(response);
        }
    }

    // Build 202 Accepted response
    let location = format!("/v2/{repository_name}/blobs/uploads/{upload_id}");

    let response = oci_cors_headers(
        Response::builder()
            .status(http::StatusCode::ACCEPTED)
            .header(http::header::LOCATION, location)
            .header(http::header::RANGE, format_upload_range(new_size))
            .header(DOCKER_UPLOAD_UUID, upload_id.to_string()),
        &[http::Method::PATCH],
    )
    .body(Body::empty())
    .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Complete an upload (with digest verification)
#[endpoint {
    method = PUT,
    path = "/v2/{name}/blobs/{ref}/{session_id}",
    tags = ["oci"],
    request_body_max_bytes = crate::OCI_REQUEST_BODY_MAX_BYTES,
}]
pub async fn oci_upload_complete(
    rqctx: RequestContext<ApiContext>,
    path: Path<UploadSessionPath>,
    query: Query<UploadCompleteQuery>,
    body: StreamingBody,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();
    validate_uploads_ref(&path.reference)?;

    // No Bearer auth required - session ID serves as authentication
    // (obtained only via authenticated POST to start upload)

    // Apply public rate limiting based on IP
    #[cfg(feature = "plus")]
    apply_public_rate_limit(&rqctx.log, context, &rqctx)?;

    let query = query.into_inner();
    let repository_name = path.name.to_string();

    // Resolve project UUID for stable storage paths
    let project_uuid = resolve_project_uuid(context, &path.name).await?;

    // Parse upload ID and expected digest
    let upload_id: UploadId = crate::error::parse_upload_id(&path.session_id)?;
    let expected_digest: Digest = crate::error::parse_digest(&query.digest)?;

    // Get storage
    let storage = context.oci_storage();

    // Validate upload belongs to this repository
    storage
        .validate_upload_repository(&upload_id, &project_uuid)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Stream optional final data to storage and complete with digest verification.
    // On error, cancel the upload session to avoid orphaned state.
    let result = async {
        stream_to_storage(body, storage, &upload_id).await?;
        storage
            .complete_upload(&upload_id, &expected_digest)
            .await
            .map_err(|e| crate::error::into_http_error(OciError::from(e)))
    }
    .await;

    let actual_digest = match result {
        Ok(digest) => digest,
        Err(e) => {
            // Best-effort cleanup of the orphaned upload session
            if let Err(cancel_err) = storage.cancel_upload(&upload_id).await {
                slog::info!(rqctx.log, "Failed to cancel upload session on error"; "upload_id" => %upload_id, "error" => %cancel_err);
            }
            return Err(e);
        },
    };

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciBlobPush);

    // Build 201 Created response
    let location = format!("/v2/{repository_name}/blobs/{actual_digest}");

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

/// Cancel an upload
#[endpoint {
    method = DELETE,
    path = "/v2/{name}/blobs/{ref}/{session_id}",
    tags = ["oci"],
}]
pub async fn oci_upload_cancel(
    rqctx: RequestContext<ApiContext>,
    path: Path<UploadSessionPath>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();
    validate_uploads_ref(&path.reference)?;

    // No Bearer auth required - session ID serves as authentication
    // (obtained only via authenticated POST to start upload)

    // Apply public rate limiting based on IP
    #[cfg(feature = "plus")]
    apply_public_rate_limit(&rqctx.log, context, &rqctx)?;

    // Resolve project UUID for stable storage paths
    let project_uuid = resolve_project_uuid(context, &path.name).await?;

    // Parse upload ID
    let upload_id: UploadId = crate::error::parse_upload_id(&path.session_id)?;

    // Get storage
    let storage = context.oci_storage();

    // Validate upload belongs to this repository
    storage
        .validate_upload_repository(&upload_id, &project_uuid)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Cancel the upload
    storage
        .cancel_upload(&upload_id)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // OCI spec requires 202 Accepted for DELETE (or 204 No Content)
    let response = oci_cors_headers(
        Response::builder().status(http::StatusCode::ACCEPTED),
        &[http::Method::DELETE],
    )
    .body(Body::empty())
    .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}
