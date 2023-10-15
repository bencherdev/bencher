use std::fmt;

use bencher_json::{urlencoded::UrlEncodedError, ThresholdUuid};
use bencher_json::{Email, ResourceId};
use bencher_plot::PlotError;
use dropshot::HttpError;
use http::StatusCode;
use once_cell::sync::Lazy;
use thiserror::Error;

#[cfg(feature = "plus")]
use crate::model::organization::OrganizationId;
use crate::{
    endpoints::Endpoint,
    model::{
        project::{branch::BranchId, testbed::TestbedId, ProjectId},
        user::UserId,
    },
};

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{0}")]
    HttpError(#[from] HttpError),

    #[error("Failed to join handle: {0}")]
    JoinHandle(tokio::task::JoinError),
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
    #[error("{0}")]
    Boundary(#[from] bencher_boundary::BoundaryError),
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
    SameUser(UserId, UserId),
    #[error("User account locked: ID {0} email {1}")]
    Locked(UserId, Email),
    #[error("Invitation email ({email}) is connected to user {email_user_id} which doesn't match {user_id}")]
    InviteEmail {
        user_id: UserId,
        email: String,
        email_user_id: UserId,
    },

    #[error("Failed to handle JWT (JSON Web Token): {0}")]
    JsonWebToken(#[from] jsonwebtoken::errors::Error),

    #[error("The branch ({branch_id}) project ID ({branch_project_id}) do not match the project ID ({project_id}).")]
    BranchProject {
        project_id: ProjectId,
        branch_id: BranchId,
        branch_project_id: ProjectId,
    },
    #[error("The testbed ({testbed_id}) project ID ({testbed_project_id}) do not match the project ID ({project_id}).")]
    TestbedProject {
        project_id: ProjectId,
        testbed_id: TestbedId,
        testbed_project_id: ProjectId,
    },
    #[error("Tried to query a private project: {0}")]
    PrivateProject(ProjectId),
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
    #[error("Bad alert status: {0}")]
    BadAlertStatus(i32),

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
    PlanMetered(OrganizationId, String),
    #[cfg(feature = "plus")]
    #[error("Organization {0} already has a licensed plan: {1}")]
    PlanLicensed(OrganizationId, String),
    #[cfg(feature = "plus")]
    #[error("Failed to parse billing ID: {0}")]
    BillingId(#[from] bencher_billing::ParseIdError),
    #[cfg(feature = "plus")]
    #[error("Failed to find metered plan for organization: {0}")]
    NoMeteredPlan(OrganizationId),
    #[cfg(feature = "plus")]
    #[error("Failed to find plan for organization: {0}")]
    NoPlanOrganization(ResourceId),
    #[cfg(feature = "plus")]
    #[error("No Biller but organization has a subscription: {0}")]
    NoBillerOrganization(ResourceId),
    #[cfg(feature = "plus")]
    #[error("No Biller but project has a subscription: {0}")]
    NoBillerProject(ProjectId),
    #[cfg(feature = "plus")]
    #[error("Organization has an inactive plan: {0}")]
    InactivePlanOrganization(ResourceId),

    #[error("Failed to cast integer: {0}")]
    IntError(#[from] std::num::TryFromIntError),
    #[error("Failed to parse integer: {0}")]
    BadInt(i64),
    #[error("Failed to parse URL encoding: {0}")]
    UrlEncoded(#[from] UrlEncodedError),
    #[error("Failed to plot data: {0}")]
    Plot(#[from] PlotError),

    #[error("Requested TTL ({requested}) is greater than max ({max})")]
    MaxTtl { requested: u32, max: u32 },
    #[error("User ({0}) cannot create a new organization")]
    CreateOrganization(UserId),
    #[error("Failed to create TLS connection for email: {0}")]
    MailTls(mail_send::Error),
    #[error("Failed to send email: {0}")]
    MailSend(mail_send::Error),
    #[error("User is not an admin: {0}")]
    Admin(UserId),
    #[error("Failed to parse organization role: {0}")]
    OrganizationRole(String),
    #[error("Failed to recognize adapter integer: {0}")]
    AdapterInt(i32),
    #[error("Failed to load statistic kind: {0}")]
    StatisticKind(i32),
    #[error("Failed to recognize visibility integer: {0}")]
    VisibilityInt(i32),
    #[error("Unexpected dimension: testbed")]
    DimensionTestbed,
    #[error("Unexpected dimension: benchmark")]
    DimensionBenchmark,
    #[error("Missing dimension: less than three")]
    DimensionMissing,

    #[error("Project ({1}) does not belong to the organization ({0})")]
    ProjectOrganizationMismatch(ResourceId, ResourceId),
    #[error("Cannot update a system Metric Kind")]
    SystemMetricKind,
    #[error("Cannot update a system Branch")]
    SystemBranch,
    #[error("Cannot update a system Testbed")]
    SystemTestbed,
    #[error("No statistic for threshold: {0}")]
    NoThresholdStatistic(ThresholdUuid),

    #[error("Failed to parse JWT (JSON Web Token): {0}")]
    Jwt(#[from] crate::context::JwtError),
    #[error("Failed to authorize RBAC (role-based access control): {0}")]
    Rbac(#[from] crate::context::RbacError),
}

impl From<ApiError> for HttpError {
    fn from(api_error: ApiError) -> Self {
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

#[derive(Debug)]
pub enum BencherResource {
    Organization,
    OrganizationRole,
    Project,
    ProjectRole,
    Report,
    MetricKind,
    Branch,
    Version,
    BranchVersion,
    Testbed,
    Benchmark,
    Perf,
    Metric,
    Threshold,
    Statistic,
    Boundary,
    Alert,
    User,
    Token,
    #[cfg(feature = "plus")]
    Plan,
}

impl fmt::Display for BencherResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Organization => "Organization",
                Self::OrganizationRole => "Organization Role",
                Self::Project => "Project",
                Self::ProjectRole => "Project Role",
                Self::Report => "Report",
                Self::MetricKind => "Metric Kind",
                Self::Branch => "Branch",
                Self::Version => "Version",
                Self::BranchVersion => "Branch Version",
                Self::Testbed => "Testbed",
                Self::Benchmark => "Benchmark",
                Self::Perf => "Perf",
                Self::Metric => "Metric",
                Self::Threshold => "Threshold",
                Self::Statistic => "Statistic",
                Self::Boundary => "Boundary",
                Self::Alert => "Alert",
                Self::User => "User",
                Self::Token => "Token",
                #[cfg(feature = "plus")]
                Self::Plan => "Plan",
            }
        )
    }
}

macro_rules! resource_not_found_err {
    // Get all
    ($resource:ident) => {
        resource_not_found_err!($resource, ())
    };
    // Get one
    ($resource:ident, $id:expr) => {
        |e| {
            crate::error::resource_not_found_error(
                &crate::error::BencherResource::$resource,
                $id,
                e,
            )
        }
    };
}

pub(crate) use resource_not_found_err;

macro_rules! resource_conflict_err {
    // Insert
    ($resource:ident, $value:expr) => {
        resource_conflict_err!($resource, (), $value)
    };
    // Update
    ($resource:ident, $id:expr, $value:expr) => {
        |e| {
            crate::error::resource_conflict_error(
                &crate::error::BencherResource::$resource,
                $id,
                &$value,
                e,
            )
        }
    };
}

pub(crate) use resource_conflict_err;

// https://developer.mozilla.org/en-US/docs/Web/HTTP/Status

pub fn bad_request_error<E>(error: E) -> HttpError
where
    E: std::fmt::Display,
{
    HttpError::for_client_error(None, StatusCode::BAD_REQUEST, error.to_string())
}

pub fn unauthorized_error<E>(error: E) -> HttpError
where
    E: std::fmt::Display,
{
    HttpError::for_client_error(None, StatusCode::UNAUTHORIZED, error.to_string())
}

pub fn payment_required_error<E>(error: E) -> HttpError
where
    E: std::fmt::Display,
{
    HttpError::for_client_error(None, StatusCode::PAYMENT_REQUIRED, error.to_string())
}

pub fn forbidden_error<E>(error: E) -> HttpError
where
    E: std::fmt::Display,
{
    HttpError::for_client_error(None, StatusCode::FORBIDDEN, error.to_string())
}

pub fn not_found_error<E>(error: E) -> HttpError
where
    E: std::fmt::Display,
{
    HttpError::for_client_error(None, StatusCode::NOT_FOUND, error.to_string())
}

pub fn conflict_error<E>(error: E) -> HttpError
where
    E: std::fmt::Display,
{
    HttpError::for_client_error(None, StatusCode::CONFLICT, error.to_string())
}

pub fn locked_error<E>(error: E) -> HttpError
where
    E: std::fmt::Display,
{
    HttpError::for_client_error(None, StatusCode::LOCKED, error.to_string())
}

pub fn resource_not_found_error<Id, E>(resource: &BencherResource, id: Id, error: E) -> HttpError
where
    Id: std::fmt::Debug,
    E: std::fmt::Display,
{
    not_found_error(format!("{resource} ({id:?}) not found: {error}",))
}

pub fn resource_conflict_error<Id, V, E>(
    resource: &BencherResource,
    id: Id,
    value: V,
    error: E,
) -> HttpError
where
    Id: std::fmt::Debug,
    V: std::fmt::Debug,
    E: std::fmt::Display,
{
    conflict_error(format!(
        "{resource} ({id:?}: {value:?}) has conflict: {error}",
    ))
}

pub fn issue_error<E>(status_code: StatusCode, title: &str, body: &str, error: E) -> HttpError
where
    E: std::fmt::Display,
{
    let error_code = uuid::Uuid::new_v4();
    let issue_url = github_issue_url(
        title,
        &format!("{body}\nError code: {error_code}\nError: {error}"),
    );
    let http_error = HttpError {
        error_code: Some(error_code.to_string()),
        status_code,
        external_message: format!("{title}: {error}\nPlease report this issue: {issue_url}"),
        internal_message: format!("INTERNAL ERROR ({error_code}): {error}"),
    };
    // debug_assert!(false, "Internal Error Found: {http_error}");
    #[cfg(feature = "sentry")]
    sentry::capture_error(&http_error);
    http_error
}

const GITHUB_ISSUE_URL_STR: &str = "https://github.com/bencherdev/bencher/issues/new";
#[allow(clippy::expect_used)]
pub static GITHUB_ISSUE_URL: Lazy<url::Url> =
    Lazy::new(|| GITHUB_ISSUE_URL_STR.parse().expect(GITHUB_ISSUE_URL_STR));

pub fn github_issue_url(title: &str, body: &str) -> url::Url {
    let mut url = GITHUB_ISSUE_URL.clone();
    let query =
        serde_urlencoded::to_string([("title", title), ("body", body), ("labels", "bug")]).ok();
    url.set_query(query.as_deref());
    url
}

#[derive(Debug, Error)]
pub enum ResourceError<Id>
where
    Id: std::fmt::Debug + std::fmt::Display,
{
    #[error(
        "{parent_resource} ID ({parent_id}) mismatch for {resource} parent ID ({expected_parent_id})"
    )]
    Mismatch {
        parent_resource: BencherResource,
        parent_id: Id,
        resource: BencherResource,
        expected_parent_id: Id,
    },
}
pub fn assert_parentage<Id>(
    parent_resource: BencherResource,
    parent_id: Id,
    resource: BencherResource,
    expected_parent_id: Id,
) where
    Id: PartialEq + std::fmt::Debug + std::fmt::Display,
{
    if parent_id != expected_parent_id {
        let err = ResourceError::Mismatch {
            parent_resource,
            parent_id,
            resource,
            expected_parent_id,
        };
        debug_assert!(false, "{err}");
        #[cfg(feature = "sentry")]
        sentry::capture_error(&err);
    }
}
