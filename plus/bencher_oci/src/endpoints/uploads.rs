//! OCI Upload Endpoints
//!
//! - POST `/v2/<name>/blobs/uploads/` - Start upload
//! - GET `/v2/<name>/blobs/uploads/<uuid>` - Get upload status
//! - PATCH `/v2/<name>/blobs/uploads/<uuid>` - Upload chunk
//! - PUT `/v2/<name>/blobs/uploads/<uuid>?digest=<digest>` - Complete upload
//! - DELETE `/v2/<name>/blobs/uploads/<uuid>` - Cancel upload

use bencher_endpoint::{CorsResponse, Delete, Endpoint, Get, Patch, Post, Put, ResponseDeleted};
use bencher_schema::context::ApiContext;
use dropshot::{Body, HttpError, Path, Query, RequestContext, UntypedBody, endpoint};
use http::Response;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::context::storage;
use crate::error::OciError;
use crate::types::{Digest, RepositoryName, UploadId};

/// Path parameters for upload start
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadStartPath {
    /// Repository name (e.g., "library/ubuntu")
    pub name: String,
}

/// Query parameters for upload start (optional mount)
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadStartQuery {
    /// Digest of blob to mount from another repository
    pub digest: Option<String>,
    /// Source repository for cross-repo mount
    pub from: Option<String>,
}

/// Path parameters for upload operations
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadPath {
    /// Repository name (e.g., "library/ubuntu")
    pub name: String,
    /// Upload session ID
    pub session_id: String,
}

/// Query parameters for upload completion
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadCompleteQuery {
    /// Expected digest of the complete blob
    pub digest: String,
}

/// CORS preflight for upload start
#[endpoint {
    method = OPTIONS,
    path = "/v2/{name}/blobs/uploads/",
    tags = ["oci"],
}]
pub async fn oci_upload_start_options(
    _rqctx: RequestContext<ApiContext>,
    _path: Path<UploadStartPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into(), Put.into()]))
}

/// Start a new blob upload
#[endpoint {
    method = POST,
    path = "/v2/{name}/blobs/uploads/",
    tags = ["oci"],
}]
pub async fn oci_upload_start(
    _rqctx: RequestContext<ApiContext>,
    path: Path<UploadStartPath>,
    query: Query<UploadStartQuery>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let query = query.into_inner();

    // Parse and validate inputs
    let repository: RepositoryName = path
        .name
        .parse()
        .map_err(|_err| HttpError::from(OciError::NameInvalid { name: path.name.clone() }))?;

    // Get storage
    let storage = storage().map_err(|e| HttpError::from(OciError::from(e)))?;

    // Handle cross-repository mount if requested
    if let (Some(digest_str), Some(from_name)) = (&query.digest, &query.from) {
        let digest: Digest = digest_str
            .parse()
            .map_err(|_err| HttpError::from(OciError::DigestInvalid { digest: digest_str.clone() }))?;
        let from_repo: RepositoryName = from_name
            .parse()
            .map_err(|_err| HttpError::from(OciError::NameInvalid { name: from_name.clone() }))?;

        // Try to mount the blob
        let mounted = storage
            .mount_blob(&from_repo, &repository, &digest)
            .await
            .map_err(|e| HttpError::from(OciError::from(e)))?;

        if mounted {
            // Mount successful - return 201 Created with Location header
            let location = format!("/v2/{repository}/blobs/{digest}");
            let response = Response::builder()
                .status(http::StatusCode::CREATED)
                .header(http::header::LOCATION, location)
                .header("Docker-Content-Digest", digest.to_string())
                .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .header(http::header::ACCESS_CONTROL_ALLOW_METHODS, "POST, PUT")
                .header(http::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type, Authorization")
                .body(Body::empty())
                .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;
            return Ok(response);
        }
        // Mount failed - fall through to start regular upload
    }

    // Start a new upload session
    let upload_id = storage
        .start_upload(&repository)
        .await
        .map_err(|e| HttpError::from(OciError::from(e)))?;

    // Build 202 Accepted response with Location header
    let location = format!("/v2/{repository}/blobs/uploads/{upload_id}");
    let response = Response::builder()
        .status(http::StatusCode::ACCEPTED)
        .header(http::header::LOCATION, location)
        .header("Range", "0-0")
        .header("Docker-Upload-UUID", upload_id.to_string())
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(http::header::ACCESS_CONTROL_ALLOW_METHODS, "POST, PUT")
        .header(http::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type, Authorization")
        .body(Body::empty())
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// CORS preflight for upload operations
#[endpoint {
    method = OPTIONS,
    path = "/v2/{name}/blobs/uploads/{session_id}",
    tags = ["oci"],
}]
pub async fn oci_upload_options(
    _rqctx: RequestContext<ApiContext>,
    _path: Path<UploadPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into(), Put.into(), Delete.into()]))
}

/// Get upload status
#[endpoint {
    method = GET,
    path = "/v2/{name}/blobs/uploads/{session_id}",
    tags = ["oci"],
}]
pub async fn oci_upload_status(
    _rqctx: RequestContext<ApiContext>,
    path: Path<UploadPath>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let repository_name = path.name.clone();

    // Parse upload ID
    let upload_id: UploadId = path
        .session_id
        .parse()
        .map_err(|_err| HttpError::from(OciError::BlobUploadUnknown { upload_id: path.session_id.clone() }))?;

    // Get storage
    let storage = storage().map_err(|e| HttpError::from(OciError::from(e)))?;

    // Get current upload size
    let size = storage
        .get_upload_size(&upload_id)
        .await
        .map_err(|e| HttpError::from(OciError::from(e)))?;

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
        .header(http::header::ACCESS_CONTROL_ALLOW_METHODS, "GET")
        .header(http::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type, Authorization")
        .body(Body::empty())
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Upload a chunk of data
#[endpoint {
    method = PATCH,
    path = "/v2/{name}/blobs/uploads/{session_id}",
    tags = ["oci"],
}]
pub async fn oci_upload_chunk(
    _rqctx: RequestContext<ApiContext>,
    path: Path<UploadPath>,
    body: UntypedBody,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let repository_name = path.name.clone();
    let data = body.as_bytes();

    // Parse upload ID
    let upload_id: UploadId = path
        .session_id
        .parse()
        .map_err(|_err| HttpError::from(OciError::BlobUploadUnknown { upload_id: path.session_id.clone() }))?;

    // Get storage
    let storage = storage().map_err(|e| HttpError::from(OciError::from(e)))?;

    // Append data to upload
    let new_size = storage
        .append_upload(&upload_id, bytes::Bytes::copy_from_slice(data))
        .await
        .map_err(|e| HttpError::from(OciError::from(e)))?;

    // Build 202 Accepted response with Location and Range headers
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
        .header(http::header::ACCESS_CONTROL_ALLOW_METHODS, "PATCH")
        .header(http::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type, Authorization")
        .body(Body::empty())
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Complete an upload (with digest verification)
#[endpoint {
    method = PUT,
    path = "/v2/{name}/blobs/uploads/{session_id}",
    tags = ["oci"],
}]
pub async fn oci_upload_complete(
    _rqctx: RequestContext<ApiContext>,
    path: Path<UploadPath>,
    query: Query<UploadCompleteQuery>,
    body: UntypedBody,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let query = query.into_inner();
    let repository_name = path.name.clone();
    let data = body.as_bytes();

    // Parse upload ID and expected digest
    let upload_id: UploadId = path
        .session_id
        .parse()
        .map_err(|_err| HttpError::from(OciError::BlobUploadUnknown { upload_id: path.session_id.clone() }))?;
    let expected_digest: Digest = query
        .digest
        .parse()
        .map_err(|_err| HttpError::from(OciError::DigestInvalid { digest: query.digest.clone() }))?;

    // Get storage
    let storage = storage().map_err(|e| HttpError::from(OciError::from(e)))?;

    // If there's data in the body, append it first
    if !data.is_empty() {
        storage
            .append_upload(&upload_id, bytes::Bytes::copy_from_slice(data))
            .await
            .map_err(|e| HttpError::from(OciError::from(e)))?;
    }

    // Complete the upload with digest verification
    let actual_digest = storage
        .complete_upload(&upload_id, &expected_digest)
        .await
        .map_err(|e| HttpError::from(OciError::from(e)))?;

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciBlobPush);

    // Build 201 Created response with Location and Docker-Content-Digest headers
    let location = format!("/v2/{repository_name}/blobs/{actual_digest}");

    let response = Response::builder()
        .status(http::StatusCode::CREATED)
        .header(http::header::LOCATION, location)
        .header("Docker-Content-Digest", actual_digest.to_string())
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(http::header::ACCESS_CONTROL_ALLOW_METHODS, "PUT")
        .header(http::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type, Authorization")
        .body(Body::empty())
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Monolithic upload (single PUT with full blob)
#[endpoint {
    method = PUT,
    path = "/v2/{name}/blobs/uploads/",
    tags = ["oci"],
}]
pub async fn oci_upload_monolithic(
    _rqctx: RequestContext<ApiContext>,
    path: Path<UploadStartPath>,
    query: Query<UploadCompleteQuery>,
    body: UntypedBody,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let query = query.into_inner();
    let data = body.as_bytes();

    // Parse inputs
    let repository: RepositoryName = path
        .name
        .parse()
        .map_err(|_err| HttpError::from(OciError::NameInvalid { name: path.name.clone() }))?;
    let expected_digest: Digest = query
        .digest
        .parse()
        .map_err(|_err| HttpError::from(OciError::DigestInvalid { digest: query.digest.clone() }))?;

    // Get storage
    let storage = storage().map_err(|e| HttpError::from(OciError::from(e)))?;

    // Start upload, append data, and complete in one operation
    let upload_id = storage
        .start_upload(&repository)
        .await
        .map_err(|e| HttpError::from(OciError::from(e)))?;

    storage
        .append_upload(&upload_id, bytes::Bytes::copy_from_slice(data))
        .await
        .map_err(|e| HttpError::from(OciError::from(e)))?;

    let actual_digest = storage
        .complete_upload(&upload_id, &expected_digest)
        .await
        .map_err(|e| HttpError::from(OciError::from(e)))?;

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciBlobPush);

    // Build 201 Created response with Location and Docker-Content-Digest headers
    let location = format!("/v2/{repository}/blobs/{actual_digest}");

    let response = Response::builder()
        .status(http::StatusCode::CREATED)
        .header(http::header::LOCATION, location)
        .header("Docker-Content-Digest", actual_digest.to_string())
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(http::header::ACCESS_CONTROL_ALLOW_METHODS, "PUT")
        .header(http::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type, Authorization")
        .body(Body::empty())
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Cancel an upload
#[endpoint {
    method = DELETE,
    path = "/v2/{name}/blobs/uploads/{session_id}",
    tags = ["oci"],
}]
pub async fn oci_upload_cancel(
    _rqctx: RequestContext<ApiContext>,
    path: Path<UploadPath>,
) -> Result<ResponseDeleted, HttpError> {
    let path = path.into_inner();

    // Parse upload ID
    let upload_id: UploadId = path
        .session_id
        .parse()
        .map_err(|_err| HttpError::from(OciError::BlobUploadUnknown { upload_id: path.session_id.clone() }))?;

    // Get storage
    let storage = storage().map_err(|e| HttpError::from(OciError::from(e)))?;

    // Cancel the upload
    storage
        .cancel_upload(&upload_id)
        .await
        .map_err(|e| HttpError::from(OciError::from(e)))?;

    Ok(Delete::auth_response_deleted())
}
