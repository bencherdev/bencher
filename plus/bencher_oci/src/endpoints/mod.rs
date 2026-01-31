//! OCI Registry Endpoints
//!
//! Implements the OCI Distribution Specification endpoints:
//! - GET `/v2/` - API version check
//! - HEAD/GET `/v2/<name>/blobs/<digest>` - Blob operations
//! - POST `/v2/<name>/blobs/uploads/` - Start blob upload
//! - PATCH `/v2/<name>/blobs/uploads/<uuid>` - Upload chunk
//! - PUT `/v2/<name>/blobs/uploads/<uuid>?digest=<digest>` - Complete upload
//! - HEAD/GET/PUT/DELETE `/v2/<name>/manifests/<reference>` - Manifest operations
//! - GET `/v2/<name>/tags/list` - List tags

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

        // Blob endpoints
        api_description.register(blobs::oci_blob_options)?;
        api_description.register(blobs::oci_blob_exists)?;
        api_description.register(blobs::oci_blob_get)?;
        api_description.register(blobs::oci_blob_delete)?;

        // Upload endpoints
        api_description.register(uploads::oci_upload_start_options)?;
        api_description.register(uploads::oci_upload_start)?;
        api_description.register(uploads::oci_upload_options)?;
        api_description.register(uploads::oci_upload_status)?;
        api_description.register(uploads::oci_upload_chunk)?;
        api_description.register(uploads::oci_upload_complete)?;
        api_description.register(uploads::oci_upload_monolithic)?;
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
