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
use crate::response::oci_cors_headers;
use bencher_endpoint::{CorsResponse, Delete, Endpoint, Get, Patch, Put};
use bencher_json::ProjectResourceId;
use bencher_oci_storage::{Digest, OciError, UploadId};
use bencher_schema::context::ApiContext;
use dropshot::{
    Body, ClientErrorStatusCode, HttpError, Path, Query, RequestContext, UntypedBody, endpoint,
};
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
    if reference != "uploads" {
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

    // Parse upload ID
    let upload_id: UploadId = path.session_id.parse().map_err(|_err| {
        crate::error::into_http_error(OciError::BlobUploadUnknown {
            upload_id: path.session_id.clone(),
        })
    })?;

    // Get storage
    let storage = context.oci_storage();

    // Validate upload belongs to this repository
    storage
        .validate_upload_repository(&upload_id, &path.name)
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
            .header("Range", format_upload_range(size))
            .header("Docker-Upload-UUID", upload_id.to_string()),
        "GET",
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
}]
pub async fn oci_upload_chunk(
    rqctx: RequestContext<ApiContext>,
    path: Path<UploadSessionPath>,
    body: UntypedBody,
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
    let data = body.as_bytes();

    // Parse upload ID
    let upload_id: UploadId = path.session_id.parse().map_err(|_err| {
        crate::error::into_http_error(OciError::BlobUploadUnknown {
            upload_id: path.session_id.clone(),
        })
    })?;

    // Get storage
    let storage = context.oci_storage();

    // Validate upload belongs to this repository
    storage
        .validate_upload_repository(&upload_id, &path.name)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Get current upload size for Content-Range validation
    let current_size = storage
        .get_upload_size(&upload_id)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Enforce max body size on cumulative upload
    let max = storage.max_body_size();
    if current_size + data.len() as u64 > max {
        return Err(crate::error::payload_too_large(
            current_size + data.len() as u64,
            max,
        ));
    }

    // Validate Content-Range header if present
    // Content-Range format can be:
    // - Standard HTTP: "bytes start-end/total" or "bytes start-end/*"
    // - OCI variant: "start-end" (just the range numbers)
    if let Some(content_range) = rqctx.request.headers().get(http::header::CONTENT_RANGE)
        && let Ok(range_str) = content_range.to_str()
    {
        // Parse the range, handling both formats
        let range_part = range_str.strip_prefix("bytes ").unwrap_or(range_str);
        // Remove trailing /total if present
        let range_nums = range_part.split_once('/').map_or(range_part, |(r, _)| r);

        if let Some((start_str, end_str)) = range_nums.split_once('-') {
            let start_ok = start_str.parse::<u64>().ok();
            let end_ok = end_str.parse::<u64>().ok();

            // Validate start offset matches current upload size
            let start_mismatch = start_ok.is_some_and(|start| start != current_size);

            // Validate end value is consistent with data length:
            // end - start + 1 should equal the body length
            let end_mismatch = match (start_ok, end_ok) {
                (Some(start), Some(end)) => {
                    let expected_len = end.saturating_sub(start) + 1;
                    expected_len != data.len() as u64
                },
                _ => false,
            };

            if start_mismatch || end_mismatch {
                // Return 416 with Location and Range headers per OCI spec
                let location = format!("/v2/{repository_name}/blobs/uploads/{upload_id}");
                let response = oci_cors_headers(
                    Response::builder()
                        .status(http::StatusCode::RANGE_NOT_SATISFIABLE)
                        .header(http::header::LOCATION, location)
                        .header("Range", format_upload_range(current_size))
                        .header("Docker-Upload-UUID", upload_id.to_string()),
                    "PATCH",
                )
                .body(Body::empty())
                .map_err(|e| {
                    HttpError::for_internal_error(format!("Failed to build response: {e}"))
                })?;
                return Ok(response);
            }
        }
    }

    // Append data to upload
    // Copy is unavoidable: Dropshot's UntypedBody only provides as_bytes() -> &[u8]
    let new_size = storage
        .append_upload(&upload_id, bytes::Bytes::copy_from_slice(data))
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Build 202 Accepted response
    let location = format!("/v2/{repository_name}/blobs/uploads/{upload_id}");

    let response = oci_cors_headers(
        Response::builder()
            .status(http::StatusCode::ACCEPTED)
            .header(http::header::LOCATION, location)
            .header("Range", format_upload_range(new_size))
            .header("Docker-Upload-UUID", upload_id.to_string()),
        "PATCH",
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
}]
pub async fn oci_upload_complete(
    rqctx: RequestContext<ApiContext>,
    path: Path<UploadSessionPath>,
    query: Query<UploadCompleteQuery>,
    body: UntypedBody,
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
    let data = body.as_bytes();

    // Parse upload ID and expected digest
    let upload_id: UploadId = path.session_id.parse().map_err(|_err| {
        crate::error::into_http_error(OciError::BlobUploadUnknown {
            upload_id: path.session_id.clone(),
        })
    })?;
    let expected_digest: Digest = query.digest.parse().map_err(|_err| {
        crate::error::into_http_error(OciError::DigestInvalid {
            digest: query.digest.clone(),
        })
    })?;

    // Get storage
    let storage = context.oci_storage();

    // Validate upload belongs to this repository
    storage
        .validate_upload_repository(&upload_id, &path.name)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // If there's data in the body, check size limit and append
    // Copy is unavoidable: Dropshot's UntypedBody only provides as_bytes() -> &[u8]
    if !data.is_empty() {
        let current_size = storage
            .get_upload_size(&upload_id)
            .await
            .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;
        let max = storage.max_body_size();
        if current_size + data.len() as u64 > max {
            return Err(crate::error::payload_too_large(
                current_size + data.len() as u64,
                max,
            ));
        }
        storage
            .append_upload(&upload_id, bytes::Bytes::copy_from_slice(data))
            .await
            .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;
    }

    // Complete the upload with digest verification
    let actual_digest = storage
        .complete_upload(&upload_id, &expected_digest)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciBlobPush);

    // Build 201 Created response
    let location = format!("/v2/{repository_name}/blobs/{actual_digest}");

    let response = oci_cors_headers(
        Response::builder()
            .status(http::StatusCode::CREATED)
            .header(http::header::LOCATION, location)
            .header("Docker-Content-Digest", actual_digest.to_string()),
        "PUT",
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

    // Parse upload ID
    let upload_id: UploadId = path.session_id.parse().map_err(|_err| {
        crate::error::into_http_error(OciError::BlobUploadUnknown {
            upload_id: path.session_id.clone(),
        })
    })?;

    // Get storage
    let storage = context.oci_storage();

    // Validate upload belongs to this repository
    storage
        .validate_upload_repository(&upload_id, &path.name)
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
        "DELETE",
    )
    .body(Body::empty())
    .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}
