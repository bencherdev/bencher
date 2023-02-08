use thiserror::Error;

#[derive(Debug, Error)]
pub enum LicenseError {
    #[error("Failed to read private pem: {0}")]
    PrivatePem(jsonwebtoken::errors::Error),
    #[error("Failed to read public pem: {0}")]
    PublicPem(jsonwebtoken::errors::Error),
}
