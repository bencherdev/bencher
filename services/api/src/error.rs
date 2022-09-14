use bencher_rbac::{Organization, Project};
use dropshot::HttpError;
use thiserror::Error;

use crate::{endpoints::Resource, model::user::auth::AuthUser};

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Failed to set global default logger")]
    SetGlobalDefault(#[from] tracing::subscriber::SetGlobalDefaultError),
    #[error("Failed to import .env file: {0}")]
    DotEnv(#[from] dotenvy::Error),
    #[error("Failed to parse role based access control (RBAC) rules: {0}")]
    Polar(oso::OsoError),
    #[error("Failed to create database connection: {0}")]
    Connection(#[from] diesel::result::ConnectionError),
    #[error("Failed to run database migrations: {0}")]
    Migrations(Box<dyn std::error::Error + Send + Sync>),
    #[error("Failed to parse IP address or port number: {0}")]
    IpAddress(#[from] std::net::AddrParseError),
    #[error("Failed to request max body size: {0}")]
    MaxBodySize(#[from] std::num::ParseIntError),
    #[error("Failed to create server logger: {0}")]
    CreateLogger(std::io::Error),
    #[error("Failed to create server: {0}")]
    CreateServer(Box<dyn std::error::Error + Send + Sync>),
    #[error("Failed to register endpoint: {0}")]
    Register(String),
    #[error("Shutting down server: {0}")]
    RunServer(String),

    #[cfg(feature = "swagger")]
    #[error("Failed to create swagger file: {0}")]
    CreateSwaggerFile(std::io::Error),
    #[cfg(feature = "swagger")]
    #[error("Failed to create swagger file: {0}")]
    WriteSwaggerFile(serde_json::Error),

    #[error("Failed to GET {}", _0.singular())]
    GetOne(Resource),
    #[error("Failed to GET {}", _0.plural())]
    GetLs(Resource),
    #[error("Failed to POST {}", _0.singular())]
    Post(Resource),
    #[error("Failed to PUT {}", _0.singular())]
    Put(Resource),
    #[error("Failed to DELETE {}", _0.singular())]
    Delete(Resource),

    #[error("Failed to parse resource ID")]
    ResourceId,
    #[error("Failed to query database: {0}")]
    Query(#[from] diesel::result::Error),
    #[error("Failed to parse UUID: {0}")]
    Uuid(#[from] uuid::Error),
    #[error("{0}")]
    AuthHeader(String),
    #[error("Invalid user: {0}")]
    User(String),
    #[error("User account locked: ID {0} email {1}")]
    Locked(i32, String),
    #[error("Failed to check permissions: {0}")]
    IsAllowed(oso::OsoError),
    #[error("Permission denied for user ({auth_user:?}) permission ({permission}) on organization ({organization:?}")]
    IsAllowedOrganization {
        auth_user: AuthUser,
        permission: bencher_rbac::organization::Permission,
        organization: Organization,
    },
    #[error("Permission denied for user ({auth_user:?}) permission ({permission}) on project ({project:?}")]
    IsAllowedProject {
        auth_user: AuthUser,
        permission: bencher_rbac::project::Permission,
        project: Project,
    },

    // TODO remove once no longer needed
    #[error(transparent)]
    Http(#[from] HttpError),
}

impl From<ApiError> for HttpError {
    fn from(error: ApiError) -> Self {
        dropshot::HttpError::for_bad_request(
            Some(http::status::StatusCode::BAD_REQUEST.to_string()),
            error.to_string(),
        )
    }
}

pub trait WordStr {
    fn singular(&self) -> &str;
    fn plural(&self) -> &str;
}

macro_rules! api_error {
    () => {
        |e| {
            let err: crate::error::ApiError = e.into();
            tracing::info!("{err}");
            err
        }
    };
}

pub(crate) use api_error;
