#![cfg(feature = "plus")]

// tempfile is used in test modules
#[cfg(test)]
use tempfile as _;

mod digest;
mod error;
mod image;
mod layer;
#[cfg(feature = "registry")]
mod registry;
mod unpack;

pub use error::OciError;
pub use image::{
    ImageConfig, ImageManifest, OciImage, digest_to_blob_path, get_manifest, parse_config,
    parse_index, parse_oci_layout,
};
pub use layer::LayerCompression;
#[cfg(feature = "registry")]
pub use registry::{ImageReference, RegistryClient};
pub use unpack::{unpack, verify_digest};

/// Map the Rust `std::env::consts::ARCH` value to the OCI architecture name.
pub(crate) fn oci_arch() -> &'static str {
    use std::env::consts::ARCH;
    match ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        arch => arch,
    }
}
