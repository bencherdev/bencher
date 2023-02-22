use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum LicenseError {
    #[error("Failed to read private pem: {0}")]
    PrivatePem(jsonwebtoken::errors::Error),
    #[error("Failed to read public pem: {0}")]
    PublicPem(jsonwebtoken::errors::Error),
    #[error("Operation not permitted when self-hosted")]
    SelfHosted,
    #[error("Failed to handle license: {0}")]
    Licensor(#[from] jsonwebtoken::errors::Error),
    #[error("Failed to validate: {0}")]
    Valid(#[from] bencher_valid::ValidError),
    #[error("Failed to cast integer: {0}")]
    IntError(#[from] std::num::TryFromIntError),
    #[error("Provided organization {0} does not match license subject organization: {1}")]
    SubjectOrganization(Uuid, Uuid),
}
