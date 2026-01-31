//! OCI Registry Endpoints
//!
//! Implements the OCI Distribution Specification endpoints using a unified
//! path structure that avoids Dropshot router conflicts.
//!
//! Path structure:
//! - GET `/v2/` - API version check
//! - `/v2/{name}/blobs/{ref}` - Blob and upload-start operations (4 segments)
//!   - GET/HEAD/DELETE when ref is a digest - blob operations
//!   - POST/PUT when ref is "uploads" - upload start
//! - `/v2/{name}/blobs/{ref}/{session_id}` - Upload session operations (5 segments)
//!   - GET/PATCH/PUT/DELETE when ref is "uploads" - upload session operations
//! - HEAD/GET/PUT/DELETE `/v2/{name}/manifests/{reference}` - Manifest operations
//! - GET `/v2/{name}/tags/list` - List tags
//!
//! This structure maintains OCI spec compliance while avoiding the Dropshot
//! router limitation of mixing literal and variable segments at the same position.

mod base;
mod blobs;
mod manifests;
mod tags;
mod uploads;

use bencher_endpoint::Registrar;
use bencher_schema::context::ApiContext;
use dropshot::{ApiDescription, ApiDescriptionRegisterError};

/// OCI API registration
pub struct Api;

impl Registrar for Api {
    fn register(
        api_description: &mut ApiDescription<ApiContext>,
        _http_options: bool,
        #[cfg(feature = "plus")] _is_bencher_cloud: bool,
    ) -> Result<(), ApiDescriptionRegisterError> {
        // Base endpoint
        api_description.register(base::oci_base_options)?;
        api_description.register(base::oci_base)?;

        // Blob and upload-start endpoints (4 segments: /v2/{name}/blobs/{ref})
        api_description.register(blobs::oci_blob_options)?;
        api_description.register(blobs::oci_blob_exists)?;
        api_description.register(blobs::oci_blob_get)?;
        api_description.register(blobs::oci_blob_delete)?;
        api_description.register(blobs::oci_upload_start)?;
        api_description.register(blobs::oci_upload_monolithic)?;

        // Upload session endpoints (5 segments: /v2/{name}/blobs/{ref}/{session_id})
        api_description.register(uploads::oci_upload_session_options)?;
        api_description.register(uploads::oci_upload_status)?;
        api_description.register(uploads::oci_upload_chunk)?;
        api_description.register(uploads::oci_upload_complete)?;
        api_description.register(uploads::oci_upload_cancel)?;

        // Manifest endpoints
        api_description.register(manifests::oci_manifest_options)?;
        api_description.register(manifests::oci_manifest_exists)?;
        api_description.register(manifests::oci_manifest_get)?;
        api_description.register(manifests::oci_manifest_put)?;
        api_description.register(manifests::oci_manifest_delete)?;

        // Tags endpoint
        api_description.register(tags::oci_tags_options)?;
        api_description.register(tags::oci_tags_list)?;

        Ok(())
    }
}

/// Public function to register OCI endpoints
pub fn register(
    api: &mut ApiDescription<ApiContext>,
) -> Result<(), ApiDescriptionRegisterError> {
    Api::register(
        api,
        true, // http_options
        #[cfg(feature = "plus")]
        false, // is_bencher_cloud - OCI endpoints don't need this distinction
    )
}
