use bencher_json::DateTime;
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
    #[error("The license term end ({term_end}) is not after it is issued ({issued_at})")]
    Term {
        issued_at: DateTime,
        term_end: DateTime,
    },
    #[error("The license is issued ({issued_at}) after now ({now})")]
    NotYetIssued { issued_at: DateTime, now: DateTime },
    #[error("The license expired ({expiration}) at or before now ({now})")]
    Expired { expiration: DateTime, now: DateTime },
    #[error(
        "Provided organization ({provided}) does not match license subject organization ({license})"
    )]
    Subject {
        provided: bencher_json::OrganizationUuid,
        license: bencher_json::OrganizationUuid,
    },
}
