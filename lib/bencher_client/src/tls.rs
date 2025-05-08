// https://github.com/astral-sh/uv/blob/591f38c25e577d29bb4fd0dde7cdb614f3129bfc/crates/uv-client/src/tls.rs

use reqwest::Identity;
use std::ffi::OsStr;
use std::io::Read as _;

#[derive(thiserror::Error, Debug)]
pub(crate) enum CertificateError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Reqwest(reqwest::Error),
}

/// Return the `Identity` from the provided file.
pub(crate) fn read_identity(ssl_client_cert: &OsStr) -> Result<Identity, CertificateError> {
    let mut buf = Vec::new();
    fs_err::File::open(ssl_client_cert)?.read_to_end(&mut buf)?;
    Identity::from_pem(&buf).map_err(|tls_err| {
        debug_assert!(tls_err.is_builder(), "must be a rustls::Error internally");
        CertificateError::Reqwest(tls_err)
    })
}
