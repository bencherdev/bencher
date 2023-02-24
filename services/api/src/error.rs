#[cfg(feature = "plus")]
use bencher_json::ResourceId;
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
    #[error("Failed to serialize/deserialize JSON: {0}")]
    SerdeJson(#[from] serde_json::Error),
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
    #[error("Failed to run stats: {0}")]
    Statrs(#[from] statrs::StatsError),
    #[error("Failed to run adapter: {0}")]
    Adapter(#[from] bencher_adapter::AdapterError),
    #[error("Failed to find env var: {0}")]
    MissingEnvVar(String),
    #[error("Failed to parse config string: {0}")]
    ParseConfigString(String),
    #[error("Failed to open config file: {0}")]
    OpenConfigFile(String),
    #[error("Failed to parse config file: {0}")]
    ParseConfigFile(String),
    #[error("Failed to write config file: {0}")]
    WriteConfigFile(String),
    #[error("Failed to serialize: {0}")]
    Serialize(serde_json::Error),
    #[error("Failed to deserialize: {0}")]
    Deserialize(serde_json::Error),
    #[error("Failed to backup file: {0}")]
    BackupFile(std::io::Error),
    #[error("Failed to configure data store: {0}")]
    DataStore(String),
    #[error("Failed to use AWS S3: {0}")]
    AwsS3(String),

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
    #[error("Failed to handle JWT (JSON Web Token): {0}")]
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
    #[error("Anonymous user tried to query private projects")]
    PrivateProjects,
    #[error("Failed to validate: {0}")]
    Valid(#[from] bencher_json::ValidError),
    #[error("Arithmetic error")]
    BadMath,
    #[error("Bad date: {0} {1} {2}")]
    BadDate(i32, u32, u32),
    #[error("Bad time: {0} {1} {2}")]
    BadTime(u32, u32, u32),

    #[cfg(not(feature = "plus"))]
    #[error("Tried to create a private project")]
    CreatePrivateProject,
    #[cfg(feature = "plus")]
    #[error("Failed to handle billing: {0}")]
    Billing(#[from] bencher_billing::BillingError),
    #[cfg(feature = "plus")]
    #[error("Failed to handle licensing: {0}")]
    License(#[from] bencher_license::LicenseError),
    #[cfg(feature = "plus")]
    #[error("Tried to init Bencher Plus for endpoint: {0}")]
    BencherPlus(url::Url),
    #[cfg(feature = "plus")]
    #[error("Tried to use a Bencher Cloud route when Self-Hosted: {0}")]
    BencherCloudOnly(String),
    #[cfg(feature = "plus")]
    #[error("Organization {0} already has a metered plan: {1}")]
    PlanMetered(i32, String),
    #[cfg(feature = "plus")]
    #[error("Organization {0} already has a licensed plan: {1}")]
    PlanLicensed(i32, String),
    #[cfg(feature = "plus")]
    #[error("Failed to parse billing ID: {0}")]
    BillingId(#[from] bencher_billing::ParseIdError),
    #[cfg(feature = "plus")]
    #[error("Failed to find metered plan for organization: {0}")]
    NoMeteredPlan(i32),
    #[cfg(feature = "plus")]
    #[error("Failed to find metered plan for organization: {0}")]
    NoMeteredPlanOrganization(ResourceId),
    #[cfg(feature = "plus")]
    #[error("Failed to find metered plan for project: {0}")]
    NoMeteredPlanProject(i32),
    #[cfg(feature = "plus")]
    #[error("No Biller but organization has a subscription: {0}")]
    NoBillerOrganization(ResourceId),
    #[cfg(feature = "plus")]
    #[error("No Biller but project has a subscription: {0}")]
    NoBillerProject(i32),
    #[cfg(feature = "plus")]
    #[error("Organization has an inactive plan: {0}")]
    InactivePlanOrganization(ResourceId),
    #[cfg(feature = "plus")]
    #[error("Project has an inactive plan: {0}")]
    InactivePlanProject(i32),

    #[error("Failed to cast integer: {0}")]
    IntError(#[from] std::num::TryFromIntError),
    #[error("Missing configuration key: {0}")]
    MissingConfigKey(String),

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
    #[error("Failed to recognize adapter integer: {0}")]
    AdapterInt(i32),
    #[error("Failed to load statistic kind: {0}")]
    StatisticKind(i32),
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
