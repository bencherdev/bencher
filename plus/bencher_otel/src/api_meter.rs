use core::fmt;
use std::sync::LazyLock;

use opentelemetry::metrics::Meter;
use uuid::Uuid;

static METER: LazyLock<Meter> = LazyLock::new(|| opentelemetry::global::meter(ApiMeter::NAME));

pub struct ApiMeter;

impl ApiMeter {
    const NAME: &str = "bencher_api";

    pub fn increment(api_counter: ApiCounter) {
        let counter = METER
            .u64_counter(api_counter.name().to_owned())
            .with_description(api_counter.description().to_owned())
            .build();
        let attributes = api_counter.attributes();
        counter.add(1, &attributes);
    }

    pub fn record(api_histogram: ApiHistogram, value: f64) {
        let histogram = METER
            .f64_histogram(api_histogram.name().to_owned())
            .with_description(api_histogram.description().to_owned())
            .with_unit(api_histogram.unit().to_owned())
            .build();
        let attributes = api_histogram.attributes();
        histogram.record(value, &attributes);
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ApiCounter {
    ServerStartup,

    OrganizationCreate,
    OrganizationDelete,

    ProjectCreate,
    ProjectDelete,

    RunClaimed,
    RunUnclaimed,

    ReportCreate,
    ReportDelete,

    MetricCreate,

    UserIp,
    UserIpNotFound,
    UserSignup(AuthMethod),
    UserLogin(AuthMethod),
    UserRecaptchaFailure,
    UserInvite,
    UserAccept(Option<AuthMethod>),
    UserConfirm,
    UserClaim,
    UserSsoJoin(AuthMethod),
    UserCheckout,

    RequestMax(IntervalKind, AuthorizationKind),

    RunClaimedMax(IntervalKind),
    RunUnclaimedMax(IntervalKind),

    UserAttemptMax(IntervalKind, AuthorizationKind),
    UserTokenMax(IntervalKind),
    UserOrganizationMax(IntervalKind),
    UserInviteMax(IntervalKind),

    Create(IntervalKind, AuthorizationKind),
    CreateMax(IntervalKind, AuthorizationKind),

    // Email
    EmailSend,

    // OCI Registry metrics
    OciBlobPush,
    OciBlobPull,
    OciManifestPush,
    OciManifestPull,
    OciTagsList,

    RunnerRequestMax(IntervalKind),

    // Runner metrics
    RunnerCreate,
    RunnerUpdate,
    RunnerTokenRotate,
    RunnerJobClaim,
    RunnerJobUpdate(JobStatusKind),

    // Self-hosted specific metrics
    SelfHostedServerStartup(Uuid),
    SelfHostedServerStats(Uuid),
}

impl ApiCounter {
    fn name(&self) -> &str {
        match self {
            Self::ServerStartup => "server.startup",

            Self::OrganizationCreate => "organization.create",
            Self::OrganizationDelete => "organization.delete",

            Self::ProjectCreate => "project.create",
            Self::ProjectDelete => "project.delete",

            Self::RunClaimed => "run.claimed",
            Self::RunUnclaimed => "run.unclaimed",

            Self::ReportCreate => "report.create",
            Self::ReportDelete => "report.delete",

            Self::MetricCreate => "metric.create",

            Self::UserIp => "user.ip",
            Self::UserIpNotFound => "user.ip.not_found",

            Self::UserSignup(_) => "user.signup",
            Self::UserLogin(_) => "user.login",
            Self::UserRecaptchaFailure => "user.recaptcha_failure",
            Self::UserInvite => "user.invite",
            Self::UserAccept(_) => "user.accept",
            Self::UserConfirm => "user.confirm",
            Self::UserClaim => "user.claim",
            Self::UserSsoJoin(_) => "user.sso.join",
            Self::UserCheckout => "user.checkout",

            Self::RequestMax(_, _) => "request.max",

            Self::RunClaimedMax(_) => "run.claimed.max",
            Self::RunUnclaimedMax(_) => "run.unclaimed.max",

            Self::UserAttemptMax(_, _) => "user.auth.max",
            Self::UserTokenMax(_) => "user.token.max",
            Self::UserOrganizationMax(_) => "user.organization.max",
            Self::UserInviteMax(_) => "user.invite.max",

            Self::Create(_, _) => "create",
            Self::CreateMax(_, _) => "create.max",

            Self::EmailSend => "email.send",

            // OCI Registry metrics
            Self::OciBlobPush => "oci.blob.push",
            Self::OciBlobPull => "oci.blob.pull",
            Self::OciManifestPush => "oci.manifest.push",
            Self::OciManifestPull => "oci.manifest.pull",
            Self::OciTagsList => "oci.tags.list",

            Self::RunnerRequestMax(_) => "runner.request.max",

            // Runner metrics
            Self::RunnerCreate => "runner.create",
            Self::RunnerUpdate => "runner.update",
            Self::RunnerTokenRotate => "runner.token.rotate",
            Self::RunnerJobClaim => "runner.job.claim",
            Self::RunnerJobUpdate(_) => "runner.job.update",

            // Self-hosted specific metrics
            Self::SelfHostedServerStartup(_) => "self_hosted.server.startup",
            Self::SelfHostedServerStats(_) => "self_hosted.server.stats",
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::ServerStartup => "Counts the number of server startups",

            Self::OrganizationCreate => "Counts the number of organization creations",
            Self::OrganizationDelete => "Counts the number of organization deletions",

            Self::ProjectCreate => "Counts the number of project creations",
            Self::ProjectDelete => "Counts the number of project deletions",

            Self::RunClaimed => "Counts the number of claimed runs",
            Self::RunUnclaimed => "Counts the number of unclaimed runs",

            Self::ReportCreate => "Counts the number of report creations",
            Self::ReportDelete => "Counts the number of report deletions",

            Self::MetricCreate => "Counts the number of metric creations",

            Self::UserIp => "Counts the number of user IP address found occurrences",
            Self::UserIpNotFound => "Counts the number of user IP address not found occurrences",

            Self::UserSignup(_) => "Counts the number of user signups",
            Self::UserLogin(_) => "Counts the number of user logins",
            Self::UserRecaptchaFailure => "Counts the number of user recaptcha failures",
            Self::UserInvite => "Counts the number of user invitations",
            Self::UserAccept(_) => "Counts the number of user acceptances",
            Self::UserConfirm => "Counts the number of user confirmations",
            Self::UserClaim => "Counts the number of user claims",
            Self::UserSsoJoin(_) => "Counts the number of user SSO joins",
            Self::UserCheckout => "Counts the number of user checkouts",

            Self::RequestMax(_, _) => "Counts the number of request maximums reached",

            Self::RunClaimedMax(_) => "Counts the number of claimed run maximums reached",
            Self::RunUnclaimedMax(_) => "Counts the number of unclaimed run maximums reached",

            Self::UserAttemptMax(_, _) => {
                "Counts the number of user authentication attempt maximums reached"
            },
            Self::UserTokenMax(_) => "Counts the number of user token maximums reached",
            Self::UserOrganizationMax(_) => {
                "Counts the number of user organization maximums reached"
            },
            Self::UserInviteMax(_) => "Counts the number of user invite maximums reached",

            Self::Create(_, _) => "Counts the number of creations",
            Self::CreateMax(_, _) => "Counts the number of creation maximums reached",

            Self::EmailSend => "Counts the number of sent emails",

            // OCI Registry metrics
            Self::OciBlobPush => "Counts the number of OCI blob pushes",
            Self::OciBlobPull => "Counts the number of OCI blob pulls",
            Self::OciManifestPush => "Counts the number of OCI manifest pushes",
            Self::OciManifestPull => "Counts the number of OCI manifest pulls",
            Self::OciTagsList => "Counts the number of OCI tags list requests",

            Self::RunnerRequestMax(_) => "Counts the number of runner request maximums reached",

            // Runner metrics
            Self::RunnerCreate => "Counts the number of runner creations",
            Self::RunnerUpdate => "Counts the number of runner updates",
            Self::RunnerTokenRotate => "Counts the number of runner token rotations",
            Self::RunnerJobClaim => "Counts the number of runner job claims",
            Self::RunnerJobUpdate(_) => "Counts the number of runner job status updates",

            // Self-hosted specific metrics
            Self::SelfHostedServerStartup(_) => "Counts the number of self-hosted server startups",
            Self::SelfHostedServerStats(_) => "Counts the number of self-hosted server stats sent",
        }
    }

    fn attributes(self) -> Vec<opentelemetry::KeyValue> {
        match self {
            Self::ServerStartup
            | Self::OrganizationCreate
            | Self::OrganizationDelete
            | Self::ProjectCreate
            | Self::ProjectDelete
            | Self::RunClaimed
            | Self::RunUnclaimed
            | Self::ReportCreate
            | Self::ReportDelete
            | Self::MetricCreate
            | Self::UserIp
            | Self::UserIpNotFound
            | Self::UserRecaptchaFailure
            | Self::UserInvite
            | Self::UserClaim
            | Self::UserCheckout
            | Self::EmailSend
            | Self::OciBlobPush
            | Self::OciBlobPull
            | Self::OciManifestPush
            | Self::OciManifestPull
            | Self::OciTagsList
            | Self::RunnerCreate
            | Self::RunnerUpdate
            | Self::RunnerTokenRotate
            | Self::RunnerJobClaim => Vec::new(),
            Self::UserSignup(auth_method)
            | Self::UserLogin(auth_method)
            | Self::UserSsoJoin(auth_method) => auth_method.attributes(),
            Self::UserAccept(auth_method) => AuthMethod::nullable_attributes(auth_method),
            Self::UserConfirm => AuthMethod::Email.attributes(),
            Self::RequestMax(interval_kind, authorization_kind)
            | Self::UserAttemptMax(interval_kind, authorization_kind)
            | Self::Create(interval_kind, authorization_kind)
            | Self::CreateMax(interval_kind, authorization_kind) => {
                vec![interval_kind.into(), authorization_kind.into()]
            },
            Self::RunnerRequestMax(interval_kind)
            | Self::RunUnclaimedMax(interval_kind)
            | Self::RunClaimedMax(interval_kind)
            | Self::UserTokenMax(interval_kind)
            | Self::UserOrganizationMax(interval_kind)
            | Self::UserInviteMax(interval_kind) => {
                vec![interval_kind.into()]
            },
            Self::RunnerJobUpdate(status_kind) => vec![status_kind.into()],
            // Self-hosted specific metrics
            Self::SelfHostedServerStartup(server_uuid)
            | Self::SelfHostedServerStats(server_uuid) => self_hosted_attributes(server_uuid),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AuthMethod {
    Email,
    OAuth(OAuthProvider),
}

impl fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Email => write!(f, "email"),
            Self::OAuth(_) => write!(f, "oauth"),
        }
    }
}

impl From<AuthMethod> for opentelemetry::KeyValue {
    fn from(auth_method: AuthMethod) -> Self {
        opentelemetry::KeyValue::new(AuthMethod::KEY, auth_method.to_string())
    }
}

impl AuthMethod {
    const KEY: &str = "auth.method";

    fn attributes(self) -> Vec<opentelemetry::KeyValue> {
        std::iter::once(self.into())
            .chain(self.provider_attribute())
            .collect()
    }

    fn nullable_attributes(auth_method: Option<Self>) -> Vec<opentelemetry::KeyValue> {
        match auth_method {
            Some(auth_method) => auth_method.attributes(),
            None => vec![opentelemetry::KeyValue::new(AuthMethod::KEY, "unknown")],
        }
    }

    fn provider_attribute(self) -> Option<opentelemetry::KeyValue> {
        match self {
            Self::Email => None,
            Self::OAuth(provider) => Some(provider.into()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OAuthProvider {
    GitHub,
    Google,
}

impl fmt::Display for OAuthProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitHub => write!(f, "github"),
            Self::Google => write!(f, "google"),
        }
    }
}

impl From<OAuthProvider> for opentelemetry::KeyValue {
    fn from(provider: OAuthProvider) -> Self {
        opentelemetry::KeyValue::new(OAuthProvider::KEY, provider.to_string())
    }
}

impl OAuthProvider {
    const KEY: &str = "auth.provider";
}

#[derive(Debug, Clone, Copy)]
pub enum IntervalKind {
    Second,
    Minute,
    Hour,
    Day,
}

impl fmt::Display for IntervalKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Second => write!(f, "second"),
            Self::Minute => write!(f, "minute"),
            Self::Hour => write!(f, "hour"),
            Self::Day => write!(f, "day"),
        }
    }
}

impl From<IntervalKind> for opentelemetry::KeyValue {
    fn from(interval_kind: IntervalKind) -> Self {
        opentelemetry::KeyValue::new(IntervalKind::KEY, interval_kind.to_string())
    }
}

impl IntervalKind {
    const KEY: &str = "interval";
}

#[derive(Debug, Clone, Copy)]
pub enum AuthorizationKind {
    Public,
    User,
}

impl fmt::Display for AuthorizationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Public => write!(f, "public"),
            Self::User => write!(f, "user"),
        }
    }
}

impl From<AuthorizationKind> for opentelemetry::KeyValue {
    fn from(authorization_kind: AuthorizationKind) -> Self {
        opentelemetry::KeyValue::new(AuthorizationKind::KEY, authorization_kind.to_string())
    }
}

impl AuthorizationKind {
    const KEY: &str = "authorization";
}

#[derive(Debug, Clone, Copy)]
pub enum JobStatusKind {
    Running,
    Completed,
    Failed,
    Canceled,
}

impl fmt::Display for JobStatusKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Canceled => write!(f, "canceled"),
        }
    }
}

impl From<JobStatusKind> for opentelemetry::KeyValue {
    fn from(status_kind: JobStatusKind) -> Self {
        opentelemetry::KeyValue::new(JobStatusKind::KEY, status_kind.to_string())
    }
}

impl JobStatusKind {
    const KEY: &str = "job.status";
}

fn self_hosted_attributes(server_uuid: Uuid) -> Vec<opentelemetry::KeyValue> {
    const KEY: &str = "server.uuid";

    vec![opentelemetry::KeyValue::new(KEY, server_uuid.to_string())]
}

#[derive(Debug, Clone, Copy)]
pub enum ApiHistogram {
    /// Time a job spent waiting in the queue before being claimed.
    JobQueueDuration(PriorityTier),
}

impl ApiHistogram {
    fn name(&self) -> &str {
        match self {
            Self::JobQueueDuration(_) => "job.queue.duration",
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::JobQueueDuration(_) => {
                "Time a job spent waiting in the queue before being claimed"
            },
        }
    }

    fn unit(&self) -> &str {
        match self {
            Self::JobQueueDuration(_) => "s",
        }
    }

    fn attributes(self) -> Vec<opentelemetry::KeyValue> {
        match self {
            Self::JobQueueDuration(tier) => vec![tier.into()],
        }
    }
}

/// Priority tier for job scheduling.
#[derive(Debug, Clone, Copy)]
pub enum PriorityTier {
    /// Enterprise tier (priority >= 300) - unlimited concurrency
    Enterprise,
    /// Team tier (priority 200-299) - unlimited concurrency
    Team,
    /// Free tier (priority 100-199) - 1 concurrent job per organization
    Free,
    /// Unclaimed tier (priority < 100) - 1 concurrent job per source IP
    Unclaimed,
}

impl fmt::Display for PriorityTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Enterprise => write!(f, "enterprise"),
            Self::Team => write!(f, "team"),
            Self::Free => write!(f, "free"),
            Self::Unclaimed => write!(f, "unclaimed"),
        }
    }
}

impl From<PriorityTier> for opentelemetry::KeyValue {
    fn from(tier: PriorityTier) -> Self {
        opentelemetry::KeyValue::new(PriorityTier::KEY, tier.to_string())
    }
}

impl PriorityTier {
    const KEY: &str = "job.priority.tier";

    /// Determine the priority tier from a priority value.
    #[must_use]
    pub fn from_priority(priority: i32) -> Self {
        if priority >= 300 {
            Self::Enterprise
        } else if priority >= 200 {
            Self::Team
        } else if priority >= 100 {
            Self::Free
        } else {
            Self::Unclaimed
        }
    }
}
