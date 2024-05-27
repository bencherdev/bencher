use std::fmt;

use dropshot::HttpError;
use http::StatusCode;
use once_cell::sync::Lazy;
use thiserror::Error;

#[derive(Debug, Clone, Copy)]
pub enum BencherResource {
    Organization,
    OrganizationRole,
    Project,
    ProjectRole,
    Report,
    ReportBenchmark,
    Plot,
    PlotBranch,
    PlotTestbed,
    PlotBenchmark,
    PlotMeasure,
    Branch,
    Version,
    BranchVersion,
    Testbed,
    Benchmark,
    Measure,
    Metric,
    Threshold,
    Model,
    Boundary,
    Alert,
    User,
    Token,
    #[cfg(feature = "plus")]
    Plan,
    #[cfg(feature = "plus")]
    Server,
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
                Self::ReportBenchmark => "Report Benchmark",
                Self::Plot => "Plot",
                Self::PlotBranch => "Plot Branch",
                Self::PlotTestbed => "Plot Testbed",
                Self::PlotBenchmark => "Plot Benchmark",
                Self::PlotMeasure => "Plot Measure",
                Self::Branch => "Branch",
                Self::Version => "Version",
                Self::BranchVersion => "Branch Version",
                Self::Testbed => "Testbed",
                Self::Benchmark => "Benchmark",
                Self::Measure => "Measure",
                Self::Metric => "Metric",
                Self::Threshold => "Threshold",
                Self::Model => "Model",
                Self::Boundary => "Boundary",
                Self::Alert => "Alert",
                Self::User => "User",
                Self::Token => "Token",
                #[cfg(feature = "plus")]
                Self::Plan => "Plan",
                #[cfg(feature = "plus")]
                Self::Server => "Server",
            }
        )
    }
}

// https://developer.mozilla.org/en-US/docs/Web/HTTP/Status

pub fn bad_request_error<E>(error: E) -> HttpError
where
    E: fmt::Display,
{
    HttpError::for_client_error(None, StatusCode::BAD_REQUEST, error.to_string())
}

pub fn unauthorized_error<E>(error: E) -> HttpError
where
    E: fmt::Display,
{
    HttpError::for_client_error(None, StatusCode::UNAUTHORIZED, error.to_string())
}

pub fn payment_required_error<E>(error: E) -> HttpError
where
    E: fmt::Display,
{
    HttpError::for_client_error(None, StatusCode::PAYMENT_REQUIRED, error.to_string())
}

pub fn forbidden_error<E>(error: E) -> HttpError
where
    E: fmt::Display,
{
    HttpError::for_client_error(None, StatusCode::FORBIDDEN, error.to_string())
}

pub fn not_found_error<E>(error: E) -> HttpError
where
    E: fmt::Display,
{
    HttpError::for_client_error(None, StatusCode::NOT_FOUND, error.to_string())
}

pub fn conflict_error<E>(error: E) -> HttpError
where
    E: fmt::Display,
{
    HttpError::for_client_error(None, StatusCode::CONFLICT, error.to_string())
}

pub fn locked_error<E>(error: E) -> HttpError
where
    E: fmt::Display,
{
    HttpError::for_client_error(None, StatusCode::LOCKED, error.to_string())
}

pub fn resource_not_found_error<V, E>(resource: BencherResource, value: V, error: E) -> HttpError
where
    V: fmt::Debug,
    E: fmt::Display,
{
    not_found_error(format!("{resource} ({value:?}) not found: {error}",))
}

pub fn resource_conflict_error<V, E>(resource: BencherResource, value: V, error: E) -> HttpError
where
    V: fmt::Debug,
    E: fmt::Display,
{
    conflict_error(format!("{resource} ({value:?}) has conflict: {error}",))
}

macro_rules! resource_not_found_err {
    // Get all
    ($resource:ident) => {
        resource_not_found_err!($resource, ())
    };
    // Get one
    ($resource:ident, $value:expr) => {
        |e| {
            #[allow(unused_qualifications)]
            crate::error::resource_not_found_error(
                crate::error::BencherResource::$resource,
                &$value,
                e,
            )
        }
    };
}

pub(crate) use resource_not_found_err;

macro_rules! resource_conflict_err {
    ($resource:ident, $value:expr) => {
        |e| {
            #[allow(unused_qualifications)]
            crate::error::resource_conflict_error(
                crate::error::BencherResource::$resource,
                &$value,
                e,
            )
        }
    };
}

pub(crate) use resource_conflict_err;

pub fn issue_error<E>(status_code: StatusCode, title: &str, body: &str, error: E) -> HttpError
where
    E: fmt::Display,
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
pub enum ParentageError<Id>
where
    Id: fmt::Debug + fmt::Display,
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
    Id: PartialEq + fmt::Debug + fmt::Display,
{
    if parent_id != expected_parent_id {
        let err = ParentageError::Mismatch {
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
