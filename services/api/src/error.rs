use std::path::PathBuf;

use bencher_rbac::{Organization, Project};
use dropshot::HttpError;
use thiserror::Error;

use crate::{endpoints::Endpoint, model::user::auth::AuthUser};

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Failed to set global default logger")]
    SetGlobalDefault(#[from] tracing::subscriber::SetGlobalDefaultError),
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
    #[error("Failed to parse default URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("Failed to inline CSS: {0}")]
    CssInline(#[from] css_inline::InlineError),
    #[error("Failed to find env var: {0}")]
    MissingEnvVar(String),
    #[error("Failed to parse config string: {0}")]
    ParseConfigString(String),
    #[error("Failed to open config file: {}", _0.display())]
    OpenConfigFile(PathBuf),
    #[error("Failed to parse config file: {}", _0.display())]
    ParseConfigFile(PathBuf),
    #[error("Failed to write config file: {}", _0.display())]
    WriteConfigFile(PathBuf),
    #[error("Failed to serialize: {0}")]
    Serialize(serde_json::Error),
    #[error("Failed to deserialize: {0}")]
    Deserialize(serde_json::Error),

    #[cfg(feature = "swagger")]
    #[error("Failed to create swagger file: {0}")]
    CreateSwaggerFile(std::io::Error),
    #[cfg(feature = "swagger")]
    #[error("Failed to create swagger file: {0}")]
    WriteSwaggerFile(serde_json::Error),

    #[error("Failed to {0}")]
    Endpoint(Endpoint),
    #[error("Failed to parse resource ID")]
    ResourceId,
    #[error("Failed to query database: {0}")]
    Query(#[from] diesel::result::Error),
    #[error("Failed to parse UUID: {0}")]
    Uuid(#[from] uuid::Error),
    #[error("Failed to parse timestamp: {0}")]
    Timestamp(i64),
    #[error("{0}")]
    AuthHeader(String),
    #[error("User is not admin and the authenticated user ({0}) does not match the requested user ({1})",)]
    SameUser(i32, i32),
    #[error("User account locked: ID {0} email {1}")]
    Locked(i32, String),
    #[error("Invitation email ({email}) is connected to user {email_user_id} which doesn't match {user_id}")]
    InviteEmail {
        user_id: i32,
        email: String,
        email_user_id: i32,
    },
    #[error("Failed to check permissions: {0}")]
    IsAllowed(oso::OsoError),
    #[error("Failed to create JWT (JSON Web Token): {0}")]
    JsonWebToken(#[from] jsonwebtoken::errors::Error),
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
    #[error("The branch ({branch_id}) project ID ({branch_project_id}) do not match the project ID ({project_id}).")]
    BranchProject {
        project_id: i32,
        branch_id: i32,
        branch_project_id: i32,
    },
    #[error("The testbed ({testbed_id}) project ID ({testbed_project_id}) do not match the project ID ({project_id}).")]
    TestbedProject {
        project_id: i32,
        testbed_id: i32,
        testbed_project_id: i32,
    },
    #[error("Tried to query a private project: {0}")]
    PrivateProject(i32),
    #[error("Invalid email: {0}")]
    Email(String),
    #[error("Requested TTL ({requested}) is greater than max ({max})")]
    MaxTtl { requested: u32, max: u32 },
    #[error("User ({0}) cannot create a new organization")]
    CreateOrganization(i32),
    #[error("Failed to create TLS connection for email: {0}")]
    MailTls(mail_send::Error),
    #[error("Failed to send email: {0}")]
    MailSend(mail_send::Error),
    #[error("User is not an admin: {0}")]
    Admin(i32),
    #[error("Failed to parse organization role: {0}")]
    OrganizationRole(String),

    // TODO remove once no longer needed
    #[error(transparent)]
    Http(#[from] HttpError),
}

impl From<ApiError> for HttpError {
    fn from(api_error: ApiError) -> Self {
        tracing::info!("{api_error}");
        dropshot::HttpError::for_bad_request(
            Some(http::status::StatusCode::BAD_REQUEST.to_string()),
            api_error.to_string(),
        )
    }
}

pub trait WordStr {
    fn singular(&self) -> &str;
    fn plural(&self) -> &str;
}

macro_rules! api_error {
    ($message:expr, $($field:tt)*) => {
        |e| {
            let err: crate::error::ApiError = e.into();
            tracing::info!("{err}");
            tracing::info!($message, $($field:tt)*);
            err
        }
    };
    ($message:expr) => {$crate::util::error::debug_error!($message,)};
    () => {
        |e| {
            let err: crate::error::ApiError = e.into();
            tracing::info!("{err}");
            err
        }
    };
}

pub(crate) use api_error;
