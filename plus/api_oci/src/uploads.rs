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
//! Note: These endpoints do NOT require Bearer token authentication.
//! The session ID itself serves as authentication - it can only be obtained
//! by authenticating to POST /v2/{name}/blobs/uploads/, and session IDs
//! are unguessable UUIDs. This matches OCI spec behavior and is required
//! for conformance test compatibility.

#[cfg(feature = "plus")]
use crate::auth::apply_public_rate_limit;
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

/// CORS preflight for upload session operations
#[endpoint {
    method = OPTIONS,
    path = "/v2/{name}/blobs/{ref}/{session_id}",
    tags = ["oci"],
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

    // Get current upload size
    let size = storage
        .get_upload_size(&upload_id)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Build 204 No Content response with Range header (OCI spec)
    let location = format!("/v2/{repository_name}/blobs/uploads/{upload_id}");
    let range = if size > 0 {
        format!("0-{}", size - 1)
    } else {
        "0-0".to_owned()
    };

    let response = Response::builder()
        .status(http::StatusCode::NO_CONTENT)
        .header(http::header::LOCATION, location)
        .header("Range", range)
        .header("Docker-Upload-UUID", upload_id.to_string())
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::empty())
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Upload a chunk of data
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

    // Get current upload size for Content-Range validation
    let current_size = storage
        .get_upload_size(&upload_id)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

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

        if let Some((start_str, _end_str)) = range_nums.split_once('-')
            && let Ok(start) = start_str.parse::<u64>()
            && start != current_size
        {
            // Return 416 with Location and Range headers per OCI spec
            let location = format!("/v2/{repository_name}/blobs/uploads/{upload_id}");
            let range = if current_size > 0 {
                format!("0-{}", current_size - 1)
            } else {
                "0-0".to_owned()
            };
            let response = Response::builder()
                .status(http::StatusCode::RANGE_NOT_SATISFIABLE)
                .header(http::header::LOCATION, location)
                .header("Range", range)
                .header("Docker-Upload-UUID", upload_id.to_string())
                .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::empty())
                .map_err(|e| {
                    HttpError::for_internal_error(format!("Failed to build response: {e}"))
                })?;
            return Ok(response);
        }
    }

    // Append data to upload
    let new_size = storage
        .append_upload(&upload_id, bytes::Bytes::copy_from_slice(data))
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Build 202 Accepted response
    let location = format!("/v2/{repository_name}/blobs/uploads/{upload_id}");
    let range = if new_size > 0 {
        format!("0-{}", new_size - 1)
    } else {
        "0-0".to_owned()
    };

    let response = Response::builder()
        .status(http::StatusCode::ACCEPTED)
        .header(http::header::LOCATION, location)
        .header("Range", range)
        .header("Docker-Upload-UUID", upload_id.to_string())
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
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

    // If there's data in the body, append it first
    if !data.is_empty() {
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

    let response = Response::builder()
        .status(http::StatusCode::CREATED)
        .header(http::header::LOCATION, location)
        .header("Docker-Content-Digest", actual_digest.to_string())
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
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

    // Cancel the upload
    storage
        .cancel_upload(&upload_id)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // OCI spec requires 202 Accepted for DELETE (or 204 No Content)
    let response = Response::builder()
        .status(http::StatusCode::ACCEPTED)
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::empty())
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}
