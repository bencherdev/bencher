use std::fmt;
use std::sync::LazyLock;

use dropshot::{ClientErrorStatusCode, ErrorStatusCode, HttpError};
use thiserror::Error;

pub const BEARER_TOKEN_FORMAT: &str = "Expected format is `Authorization: Bearer <bencher.api.token>`. Where `<bencher.api.token>` is your Bencher API token.";
pub const BEARER_AUTH_FORMAT: &str = "Expected format is `Authorization: Bearer <token>`. Where `<token>` is your Bencher API token or project API key (`bencher_run_...`).";

#[derive(Debug, Clone, Copy)]
pub enum BencherResource {
    Organization,
    OrganizationRole,
    Project,
    ProjectRole,
    ProjectKey,
    Report,
    ReportBenchmark,
    Plot,
    PlotBranch,
    PlotTestbed,
    PlotBenchmark,
    PlotMeasure,
    Branch,
    Head,
    Version,
    HeadVersion,
    Testbed,
    Benchmark,
    Measure,
    Metric,
    Threshold,
    Model,
    Boundary,
    Alert,
    Runner,
    RunnerSpec,
    Spec,
    Job,
    User,
    Token,
    #[cfg(feature = "plus")]
    Plan,
    #[cfg(feature = "plus")]
    Sso,
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
                Self::ProjectKey => "Project Key",
                Self::Report => "Report",
                Self::ReportBenchmark => "Report Benchmark",
                Self::Plot => "Plot",
                Self::PlotBranch => "Plot Branch",
                Self::PlotTestbed => "Plot Testbed",
                Self::PlotBenchmark => "Plot Benchmark",
                Self::PlotMeasure => "Plot Measure",
                Self::Branch => "Branch",
                Self::Head => "Head",
                Self::Version => "Version",
                Self::HeadVersion => "Head Version",
                Self::Testbed => "Testbed",
                Self::Benchmark => "Benchmark",
                Self::Measure => "Measure",
                Self::Metric => "Metric",
                Self::Threshold => "Threshold",
                Self::Model => "Model",
                Self::Boundary => "Boundary",
                Self::Alert => "Alert",
                Self::Runner => "Runner",
                Self::RunnerSpec => "Runner Spec",
                Self::Spec => "Spec",
                Self::Job => "Job",
                Self::User => "User",
                Self::Token => "Token",
                #[cfg(feature = "plus")]
                Self::Plan => "Plan",
                #[cfg(feature = "plus")]
                Self::Sso => "SSO",
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
    cors_headers(HttpError::for_client_error(
        None,
        ClientErrorStatusCode::BAD_REQUEST,
        error.to_string(),
    ))
}

pub fn unauthorized_error<E>(error: E) -> HttpError
where
    E: fmt::Display,
{
    cors_headers(HttpError::for_client_error(
        None,
        ClientErrorStatusCode::UNAUTHORIZED,
        error.to_string(),
    ))
}

pub fn payment_required_error<E>(error: E) -> HttpError
where
    E: fmt::Display,
{
    cors_headers(HttpError::for_client_error(
        None,
        ClientErrorStatusCode::PAYMENT_REQUIRED,
        error.to_string(),
    ))
}

pub fn forbidden_error<E>(error: E) -> HttpError
where
    E: fmt::Display,
{
    cors_headers(HttpError::for_client_error(
        None,
        ClientErrorStatusCode::FORBIDDEN,
        error.to_string(),
    ))
}

pub fn not_found_error<E>(error: E) -> HttpError
where
    E: fmt::Display,
{
    cors_headers(HttpError::for_client_error(
        None,
        ClientErrorStatusCode::NOT_FOUND,
        error.to_string(),
    ))
}

pub fn request_timeout_error<E>(error: E) -> HttpError
where
    E: fmt::Display,
{
    cors_headers(HttpError::for_client_error(
        None,
        ClientErrorStatusCode::REQUEST_TIMEOUT,
        error.to_string(),
    ))
}

pub fn conflict_error<E>(error: E) -> HttpError
where
    E: fmt::Display,
{
    cors_headers(HttpError::for_client_error(
        None,
        ClientErrorStatusCode::CONFLICT,
        error.to_string(),
    ))
}

pub fn is_not_found(error: &HttpError) -> bool {
    error.status_code == ClientErrorStatusCode::NOT_FOUND
}

pub fn is_conflict(error: &HttpError) -> bool {
    error.status_code == ClientErrorStatusCode::CONFLICT
}

pub fn locked_error<E>(error: E) -> HttpError
where
    E: fmt::Display,
{
    cors_headers(HttpError::for_client_error(
        None,
        ClientErrorStatusCode::LOCKED,
        error.to_string(),
    ))
}

pub fn too_many_requests<E>(error: E) -> HttpError
where
    E: fmt::Display,
{
    cors_headers(HttpError::for_client_error(
        None,
        ClientErrorStatusCode::TOO_MANY_REQUESTS,
        error.to_string(),
    ))
}

/// A transient, retryable server condition (e.g. `SQLITE_BUSY`, "database is locked"): the request
/// was well-formed but the server could not service it right now. Returns `503 Service Unavailable`
/// with a `Retry-After` hint. The CLI (`bencher_client`) and Console retry 5xx with their own
/// backoff and do not read `Retry-After`; the header is included for HTTP correctness and other
/// clients. The detailed cause goes to `internal_message` (and Sentry at the call site), not to the
/// user-facing `external_message`.
pub fn service_unavailable_error<E>(error: E) -> HttpError
where
    E: fmt::Display,
{
    let mut http_error = HttpError {
        status_code: ErrorStatusCode::SERVICE_UNAVAILABLE,
        error_code: None,
        external_message: "Service temporarily unavailable, please retry.".to_owned(),
        internal_message: error.to_string(),
        headers: None,
    };
    if let Err(err) = http_error.add_header("retry-after", "1") {
        debug_assert!(false, "{err}");
        #[cfg(feature = "sentry")]
        sentry::capture_error(&err);
    }
    cors_headers(http_error)
}

pub fn resource_not_found_error<V, E>(resource: BencherResource, value: V, error: E) -> HttpError
where
    V: fmt::Debug,
    E: fmt::Display,
{
    not_found_error(format!(
        "{resource} ({value:?}) not found: {error}\n{resource} may be private and require authentication or it may not exist."
    ))
}

/// Visibility × authentication state for project access denials.
/// Encodes the four combinations so callers cannot accidentally produce the
/// wrong HTTP status for a visibility/actor pair.
pub enum ProjectAuthState {
    /// Anonymous request on a public project requiring authentication → 401.
    PublicAnonymous,
    /// Authenticated principal on a public project lacking permission → 403.
    PublicAuthenticated,
    /// Anonymous request on a private project → 404 (info-hide, same as nonexistent).
    PrivateAnonymous,
    /// Authenticated non-member on a private project → 404 (info-hide).
    PrivateAuthenticated,
}

impl ProjectAuthState {
    pub fn new(is_public: bool, is_authenticated: bool) -> Self {
        match (is_public, is_authenticated) {
            (true, false) => Self::PublicAnonymous,
            (true, true) => Self::PublicAuthenticated,
            (false, false) => Self::PrivateAnonymous,
            (false, true) => Self::PrivateAuthenticated,
        }
    }

    /// Build the HTTP error for this access denial.
    /// Private projects must return the info-hiding 404 — callers should not
    /// be able to distinguish "private" from "nonexistent" without authorization.
    pub fn auth_error<V, E>(self, value: V, error: E) -> HttpError
    where
        V: fmt::Debug,
        E: fmt::Display,
    {
        match self {
            Self::PublicAnonymous => unauthorized_error(format!(
                "Authentication required to {error} {resource} ({value:?})",
                resource = BencherResource::Project,
            )),
            Self::PublicAuthenticated => forbidden_error(format!(
                "{resource} ({value:?}) access denied: {error}. This {resource} is public but you do not have the required permission.",
                resource = BencherResource::Project,
            )),
            Self::PrivateAnonymous | Self::PrivateAuthenticated => {
                resource_not_found_error(BencherResource::Project, value, error)
            },
        }
    }
}

pub fn with_token_hint(mut err: HttpError) -> HttpError {
    if err.status_code == ClientErrorStatusCode::NOT_FOUND {
        err.external_message = format!("{}\n{BEARER_TOKEN_FORMAT}", err.external_message);
        err.internal_message = format!("{}\n{BEARER_TOKEN_FORMAT}", err.internal_message);
    }
    err
}

pub fn with_auth_hint(mut err: HttpError) -> HttpError {
    if err.status_code == ClientErrorStatusCode::NOT_FOUND {
        err.external_message = format!("{}\n{BEARER_AUTH_FORMAT}", err.external_message);
        err.internal_message = format!("{}\n{BEARER_AUTH_FORMAT}", err.internal_message);
    }
    err
}

pub fn resource_conflict_error<V, E>(resource: BencherResource, value: V, error: E) -> HttpError
where
    V: fmt::Debug,
    E: fmt::Debug + fmt::Display,
{
    let database_is_locked = error.to_string().contains("database is locked");
    let err = ResourceError::Conflict {
        resource,
        value,
        error,
    };
    if database_is_locked {
        // `SQLITE_BUSY` ("database is locked") is NOT a real conflict: the write never reached a
        // conflicting state, the writer just could not acquire the lock within `busy_timeout`.
        // This should never happen (see disaster-recovery WAL tuning), so KEEP the Sentry tripwire
        // for visibility — but return a retryable `503` so clients retry instead of the request
        // failing with a non-retryable `409`.
        debug_assert!(false, "{err}");
        #[cfg(feature = "sentry")]
        sentry::capture_error(&err);
        service_unavailable_error(err)
    } else {
        conflict_error(err)
    }
}

#[derive(Debug, Error)]
pub enum ResourceError<V, E>
where
    V: fmt::Debug,
    E: fmt::Debug + fmt::Display,
{
    #[error("{resource} ({value:?}) has conflict: {error}")]
    Conflict {
        resource: BencherResource,
        value: V,
        error: E,
    },
}

#[macro_export]
macro_rules! resource_not_found_err {
    // Get all
    ($resource:ident) => {
        resource_not_found_err!($resource, ())
    };
    // Get one
    ($resource:ident, $value:expr) => {
        |e| {
            $crate::error::resource_not_found_error(
                $crate::error::BencherResource::$resource,
                &$value,
                e,
            )
        }
    };
}

pub use resource_not_found_err;

#[macro_export]
macro_rules! resource_conflict_err {
    ($resource:ident, $value:expr) => {
        |e| {
            $crate::error::resource_conflict_error(
                $crate::error::BencherResource::$resource,
                &$value,
                e,
            )
        }
    };
}

pub use resource_conflict_err;

const ALL_ORIGIN: &str = "*";
const ALLOW_HEADERS: &str = "Content-Type, Authorization";
const EXPOSE_HEADERS: &str = "X-Total-Count";

pub fn issue_error<E>(title: &str, body: &str, error: E) -> HttpError
where
    E: fmt::Display,
{
    let error_code = uuid::Uuid::new_v4();
    let issue_url = github_issue_url(
        title,
        &format!("{body}\nError code: {error_code}\nError: {error}"),
    );
    let http_error = HttpError {
        status_code: ErrorStatusCode::INTERNAL_SERVER_ERROR,
        error_code: Some(error_code.to_string()),
        external_message: format!("{title}: {error}\nPlease report this issue: {issue_url}"),
        internal_message: format!("INTERNAL ERROR ({error_code}): {error}"),
        headers: None,
    };
    // debug_assert!(false, "Internal Error Found: {http_error}");
    #[cfg(feature = "sentry")]
    sentry::capture_error(&http_error);
    cors_headers(http_error)
}

fn cors_headers(mut http_error: HttpError) -> HttpError {
    for (header, value) in [
        ("access-control-allow-origin", ALL_ORIGIN),
        (
            "access-control-allow-methods",
            "GET, POST, PUT, PATCH, DELETE, OPTIONS",
        ),
        ("access-control-allow-headers", ALLOW_HEADERS),
        ("access-control-expose-headers", EXPOSE_HEADERS),
    ] {
        if let Err(err) = http_error.add_header(header, value) {
            debug_assert!(false, "{err}");
            #[cfg(feature = "sentry")]
            sentry::capture_error(&err);
        }
    }
    http_error
}

const GITHUB_ISSUE_URL_STR: &str = "https://github.com/bencherdev/bencher/issues/new";
#[expect(clippy::expect_used, reason = "valid constant URL")]
pub static GITHUB_ISSUE_URL: LazyLock<url::Url> =
    LazyLock::new(|| GITHUB_ISSUE_URL_STR.parse().expect(GITHUB_ISSUE_URL_STR));

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
    Parentage {
        parent_resource: BencherResource,
        parent_id: Id,
        resource: BencherResource,
        expected_parent_id: Id,
    },
    #[error(
        "{parent_resource} ID is not the same for {left_resource} ID ({left_parent_id}) and {right_resource} ({right_parent_id})"
    )]
    Siblings {
        parent_resource: BencherResource,
        left_resource: BencherResource,
        left_parent_id: Id,
        right_resource: BencherResource,
        right_parent_id: Id,
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
        let err = ParentageError::Parentage {
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

pub fn assert_siblings<Id>(
    parent_resource: BencherResource,
    left_resource: BencherResource,
    left_parent_id: Id,
    right_resource: BencherResource,
    right_parent_id: Id,
) where
    Id: PartialEq + fmt::Debug + fmt::Display,
{
    if left_parent_id != right_parent_id {
        let err = ParentageError::Siblings {
            parent_resource,
            left_resource,
            left_parent_id,
            right_resource,
            right_parent_id,
        };
        debug_assert!(false, "{err}");
        #[cfg(feature = "sentry")]
        sentry::capture_error(&err);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_conflict_true_for_conflict_error() {
        let error = conflict_error("duplicate resource");
        assert!(is_conflict(&error));
    }

    // A transient `SQLITE_BUSY` ("database is locked") is reclassified to a retryable 503,
    // not a 409 conflict, so clients retry instead of dropping the request.
    #[test]
    fn service_unavailable_error_is_retryable_503() {
        let error = service_unavailable_error("database is locked");
        assert!(error.status_code == ErrorStatusCode::SERVICE_UNAVAILABLE);
        assert!(!is_conflict(&error));
    }

    #[test]
    fn is_conflict_false_for_not_found_error() {
        let error = not_found_error("missing");
        assert!(!is_conflict(&error));
    }

    #[test]
    fn is_conflict_false_for_bad_request_error() {
        let error = bad_request_error("bad input");
        assert!(!is_conflict(&error));
    }

    #[test]
    fn is_conflict_true_for_resource_conflict_error() {
        let error =
            resource_conflict_error(BencherResource::Project, "test-project", "UNIQUE failed");
        assert!(is_conflict(&error));
    }

    #[test]
    fn project_auth_state_public_authenticated_returns_forbidden() {
        let error = ProjectAuthState::new(true, true).auth_error("my-project", "view");
        assert_eq!(error.status_code, ClientErrorStatusCode::FORBIDDEN);
        assert!(
            error.external_message.contains("access denied"),
            "expected access denied message, got: {}",
            error.external_message
        );
        assert!(
            error.external_message.contains("public"),
            "expected public hint, got: {}",
            error.external_message
        );
    }

    #[test]
    fn project_auth_state_public_anonymous_returns_unauthorized() {
        let error = ProjectAuthState::new(true, false).auth_error("my-project", "view");
        assert_eq!(error.status_code, ClientErrorStatusCode::UNAUTHORIZED);
        assert!(
            error.external_message.contains("Authentication required"),
            "expected Authentication required wording, got: {}",
            error.external_message
        );
    }

    #[test]
    fn project_auth_state_private_authenticated_returns_not_found_with_info_hiding() {
        let error = ProjectAuthState::new(false, true).auth_error("my-project", "view");
        assert_eq!(error.status_code, ClientErrorStatusCode::NOT_FOUND);
        assert!(
            error.external_message.contains("may be private"),
            "expected info-hiding wording, got: {}",
            error.external_message
        );
    }

    #[test]
    fn project_auth_state_private_anonymous_returns_not_found_with_info_hiding() {
        let error = ProjectAuthState::new(false, false).auth_error("my-project", "view");
        assert_eq!(error.status_code, ClientErrorStatusCode::NOT_FOUND);
        assert!(
            error.external_message.contains("may be private"),
            "expected info-hiding wording, got: {}",
            error.external_message
        );
    }
}
