use thiserror::Error;

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
    Valid(#[from] bencher_json::ValidError),
    #[error("Failed to cast integer: {0}")]
    IntError(#[from] std::num::TryFromIntError),
    #[error(
        "Provided organization ({provided}) does not match license subject organization ({license})"
    )]
    Subject {
        provided: bencher_json::OrganizationUuid,
        license: bencher_json::OrganizationUuid,
    },
    #[error("The usage ({usage}) exceeds the license entitlements ({entitlements})")]
    Entitlements { usage: u64, entitlements: u64 },
}
