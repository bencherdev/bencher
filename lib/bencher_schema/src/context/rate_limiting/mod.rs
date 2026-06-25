use std::{io::Write as _, net::IpAddr, path::Path, time::Duration};

use camino::{Utf8Path, Utf8PathBuf};

use bencher_json::{
    DateTime, OrganizationUuid, PlanLevel, ProjectUuid, RunnerUuid, UserUuid,
    system::config::JsonRateLimiting,
};
use bencher_license::Licensor;
#[cfg(feature = "otel")]
use bencher_otel::AuthorizationKind;
use dropshot::HttpError;
pub use http::HeaderMap;
use slog::Logger;

use crate::{
    error::{BencherResource, too_many_requests},
    model::{
        organization::{QueryOrganization, plan::LicenseUsage},
        project::{QueryProject, branch::QueryBranch, threshold::QueryThreshold},
    },
};

mod bandwidth;
mod project;
mod public;
mod remote_ip;
mod runner;
pub(super) mod snapshot;
mod user;

use bandwidth::BandwidthRateLimiter;
use project::ProjectRateLimiter;
use public::PublicRateLimiter;
use runner::RunnerRateLimiter;
use snapshot::RateLimitingSnapshot;
use user::UserRateLimiter;

use super::DbConnection;

const DEFAULT_UNCLAIMED_LIMIT: u32 = u8::MAX as u32;
const DEFAULT_CLAIMED_LIMIT: u32 = u16::MAX as u32;

pub struct RateLimiting {
    // Database-backed rate limits
    window: Duration,
    unclaimed_limit: u32,
    claimed_limit: u32,
    // In-memory rate limiters
    public: PublicRateLimiter,
    user: UserRateLimiter,
    project: ProjectRateLimiter,
    runner: RunnerRateLimiter,
    bandwidth: BandwidthRateLimiter,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum RateLimitingError {
    #[error("Organization ({uuid}) has exceeded the daily rate limit ({rate_limit}) for {resource} creation. Please, reduce your daily usage.", uuid = organization.uuid)]
    Organization {
        organization: QueryOrganization,
        resource: BencherResource,
        rate_limit: u32,
    },
    #[error("Unclaimed organization ({uuid}) has exceeded the daily rate limit ({rate_limit}). Please, reduce your daily usage or claim the organization: https://bencher.dev/auth/signup?claim={uuid}", uuid = organization.uuid)]
    UnclaimedOrganization {
        organization: QueryOrganization,
        rate_limit: u32,
    },
    #[error("No plan (subscription or license) found for claimed organization ({uuid}) that exceeds the daily rate limit ({rate_limit}). Please, reduce your daily usage or purchase a Bencher Plus plan: https://bencher.dev/pricing", uuid = organization.uuid)]
    ClaimedOrganization {
        organization: QueryOrganization,
        rate_limit: u32,
    },
    #[error("Unclaimed project ({uuid}) has exceeded the daily rate limit ({rate_limit}) for {resource} creation. Please, reduce your daily usage or claim the project: https://bencher.dev/auth/signup?claim={uuid}", uuid = project.uuid)]
    UnclaimedProject {
        project: QueryProject,
        resource: BencherResource,
        rate_limit: u32,
    },
    #[error("Claimed project ({uuid}) has exceeded the daily rate limit ({rate_limit}) for {resource} creation. Please, reduce your daily usage.", uuid = project.uuid)]
    ClaimedProject {
        project: QueryProject,
        resource: BencherResource,
        rate_limit: u32,
    },
    #[error("Branch ({uuid}) has exceeded the daily rate limit ({rate_limit}) for {resource} creation. Please, reduce your daily usage.", uuid = branch.uuid)]
    Branch {
        branch: QueryBranch,
        resource: BencherResource,
        rate_limit: u32,
    },
    #[error("Threshold ({uuid}) has exceeded the daily rate limit ({rate_limit}) for {resource} creation. Please, reduce your daily usage.", uuid = threshold.uuid)]
    Threshold {
        threshold: QueryThreshold,
        resource: BencherResource,
        rate_limit: u32,
    },

    #[error("Organization ({uuid}) has exceeded the daily OCI bandwidth limit ({limit_gib} GiB). Please reduce usage or upgrade: https://bencher.dev/pricing", uuid = organization.uuid)]
    OciBandwidth {
        organization: QueryOrganization,
        limit_gib: u64,
    },

    #[error("Too many requests for project per {0}. Please, try again later.")]
    ProjectRequests(bencher_rate_limiter::Interval),
    #[error("Too many runs for project per {0}. Please, try again later.")]
    ProjectRuns(bencher_rate_limiter::Interval),

    #[error("Too many requests for runner per {0}. Please, try again later.")]
    RunnerRequests(bencher_rate_limiter::Interval),

    #[error("Too many requests for IP address per {0}. Please, try again later.")]
    IpAddressRequests(bencher_rate_limiter::Interval),
    #[error("Too many authentication attempts for IP address per {0}. Please, try again later.")]
    IpAddressAttempts(bencher_rate_limiter::Interval),
    #[error(
        "Too many runs from unclaimed IP address per {0}. Please, claim the project or try again later."
    )]
    UnclaimedRun(bencher_rate_limiter::Interval),

    #[error("Too many requests for user per {0}. Please, try again later.")]
    UserRequests(bencher_rate_limiter::Interval),
    #[error("Too many runs for user per {0}. Please, try again later.")]
    UserRuns(bencher_rate_limiter::Interval),
    #[error("Too many authentication attempts for user per {0}. Please, try again later.")]
    UserAttempts(bencher_rate_limiter::Interval),
    #[error("Too many credential generations for user per {0}. Please, try again later.")]
    UserCredentials(bencher_rate_limiter::Interval),
    #[error("Too many organization creations for user per {0}. Please, try again later.")]
    UserOrganizations(bencher_rate_limiter::Interval),
    #[error("Too many invitation emails for user per {0}. Please, try again later.")]
    UserInvites(bencher_rate_limiter::Interval),
}

impl Default for RateLimiting {
    fn default() -> Self {
        Self {
            window: bencher_rate_limiter::DAY,
            unclaimed_limit: DEFAULT_UNCLAIMED_LIMIT,
            claimed_limit: DEFAULT_CLAIMED_LIMIT,
            public: PublicRateLimiter::default(),
            user: UserRateLimiter::default(),
            project: ProjectRateLimiter::default(),
            runner: RunnerRateLimiter::default(),
            bandwidth: BandwidthRateLimiter::default(),
        }
    }
}

impl From<JsonRateLimiting> for RateLimiting {
    fn from(json: JsonRateLimiting) -> Self {
        let JsonRateLimiting {
            window,
            unclaimed_limit,
            claimed_limit,
            public,
            user,
            project,
            runner,
            oci_bandwidth,
        } = json;
        Self {
            window: window
                .map(u64::from)
                .map_or(bencher_rate_limiter::DAY, Duration::from_secs),
            unclaimed_limit: unclaimed_limit.unwrap_or(DEFAULT_UNCLAIMED_LIMIT),
            claimed_limit: claimed_limit.unwrap_or(DEFAULT_CLAIMED_LIMIT),
            public: public.map_or_else(PublicRateLimiter::default, Into::into),
            user: user.map_or_else(UserRateLimiter::default, Into::into),
            project: project.map_or_else(ProjectRateLimiter::default, Into::into),
            runner: runner.map_or_else(RunnerRateLimiter::default, Into::into),
            bandwidth: oci_bandwidth.map_or_else(BandwidthRateLimiter::default, Into::into),
        }
    }
}

impl RateLimiting {
    pub fn new(
        log: &Logger,
        conn: &mut DbConnection,
        licensor: &Licensor,
        is_bencher_cloud: bool,
        rate_limiting: Option<JsonRateLimiting>,
    ) -> Self {
        match (is_bencher_cloud, rate_limiting) {
            (true, Some(json_rate_limiting)) => {
                slog::info!(log, "Applying custom rate limits for Bencher Cloud");
                json_rate_limiting.into()
            },
            (true, None) => {
                slog::info!(log, "Applying default rate limits for Bencher Cloud");
                Self::default()
            },
            (false, Some(json_rate_limiting)) => {
                match LicenseUsage::get_for_server(conn, licensor, Some(PlanLevel::Team)) {
                    Ok(license_usages) if license_usages.is_empty() => {
                        slog::warn!(
                            log,
                            "Custom rate limits provided, but there is no valid Bencher Plus license key! Please purchase a license key: https://bencher.dev/pricing"
                        );
                        Self::max()
                    },
                    Ok(_) => {
                        slog::info!(log, "Applying custom rate limits for Bencher Self-Hosted");
                        json_rate_limiting.into()
                    },
                    Err(e) => {
                        slog::error!(log, "Failed to check license for custom rate limits: {e}");
                        Self::max()
                    },
                }
            },
            (false, None) => {
                slog::info!(log, "No rate limits applied for Bencher Self-Hosted");
                Self::max()
            },
        }
    }

    pub fn max() -> Self {
        Self {
            window: bencher_rate_limiter::DAY,
            unclaimed_limit: u32::MAX,
            claimed_limit: u32::MAX,
            public: PublicRateLimiter::max(),
            user: UserRateLimiter::max(),
            project: ProjectRateLimiter::max(),
            runner: RunnerRateLimiter::max(),
            bandwidth: BandwidthRateLimiter::max(),
        }
    }

    pub fn prune(&self) {
        self.public.prune();
        self.user.prune();
        self.project.prune();
        self.runner.prune();
        self.bandwidth.prune();
    }

    pub fn window(&self) -> (DateTime, DateTime) {
        let end_time = chrono::Utc::now();
        let start_time = end_time - self.window;
        (start_time.into(), end_time.into())
    }

    pub fn check_claimable_limit<FUn, FCl>(
        &self,
        is_claimed: bool,
        window_usage: u32,
        unclaimed_error_fn: FUn,
        claimed_error_fn: FCl,
    ) -> Result<(), HttpError>
    where
        FUn: FnOnce(u32) -> RateLimitingError,
        FCl: FnOnce(u32) -> RateLimitingError,
    {
        if is_claimed {
            self.check_claimed_limit(window_usage, claimed_error_fn)
        } else {
            Self::check_inner(
                self.unclaimed_limit,
                window_usage,
                unclaimed_error_fn,
                #[cfg(feature = "otel")]
                AuthorizationKind::Public,
            )
        }
    }

    pub fn check_claimed_limit<F>(&self, window_usage: u32, error_fn: F) -> Result<(), HttpError>
    where
        F: FnOnce(u32) -> RateLimitingError,
    {
        Self::check_inner(
            self.claimed_limit,
            window_usage,
            error_fn,
            #[cfg(feature = "otel")]
            AuthorizationKind::User,
        )
    }

    fn check_inner<F>(
        limit: u32,
        window_usage: u32,
        error_fn: F,
        #[cfg(feature = "otel")] authorization_kind: AuthorizationKind,
    ) -> Result<(), HttpError>
    where
        F: FnOnce(u32) -> RateLimitingError,
    {
        if window_usage < limit {
            Ok(())
        } else {
            Err(too_many_requests(error_fn(limit)))
        }
        .inspect(|()| {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::Create(authorization_kind));
        })
        .inspect_err(|_| {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::CreateMax(
                authorization_kind,
            ));
        })
    }

    pub fn public_request(&self, remote_ip: IpAddr) -> Result<(), HttpError> {
        self.public.check_request(remote_ip)
    }

    pub fn public_auth_attempt(&self, remote_ip: IpAddr) -> Result<(), HttpError> {
        self.public.check_attempt(remote_ip)
    }

    pub fn unclaimed_run(&self, remote_ip: IpAddr) -> Result<(), HttpError> {
        self.public.check_run(remote_ip)
    }

    pub fn user_request(&self, user_uuid: UserUuid) -> Result<(), HttpError> {
        self.user.check_request(user_uuid)
    }

    pub fn auth_attempt(&self, user_uuid: UserUuid) -> Result<(), HttpError> {
        self.user.check_attempt(user_uuid)
    }

    pub fn create_credential(&self, user_uuid: UserUuid) -> Result<(), HttpError> {
        self.user.check_credential(user_uuid)
    }

    pub fn create_organization(&self, user_uuid: UserUuid) -> Result<(), HttpError> {
        self.user.check_organization(user_uuid)
    }

    pub fn user_invite(&self, user_uuid: UserUuid) -> Result<(), HttpError> {
        self.user.check_invite(user_uuid)
    }

    pub fn claimed_run(&self, user_uuid: UserUuid) -> Result<(), HttpError> {
        self.user.check_run(user_uuid)
    }

    pub fn project_request(&self, project_uuid: ProjectUuid) -> Result<(), HttpError> {
        self.project.check_request(project_uuid)
    }

    pub fn project_run(&self, project_uuid: ProjectUuid) -> Result<(), HttpError> {
        self.project.check_run(project_uuid)
    }

    pub fn runner_request(&self, runner_uuid: RunnerUuid) -> Result<(), HttpError> {
        self.runner.check_request(runner_uuid)
    }

    pub fn check_oci_bandwidth(
        &self,
        org_uuid: OrganizationUuid,
        priority: bencher_json::Priority,
        organization: &QueryOrganization,
    ) -> Result<(), HttpError> {
        self.bandwidth.check(org_uuid, priority, organization)
    }

    pub fn record_oci_bandwidth(&self, org_uuid: OrganizationUuid, bytes: u64) {
        self.bandwidth.record(org_uuid, bytes);
    }

    pub fn remote_ip(log: &Logger, headers: &HeaderMap) -> Option<IpAddr> {
        remote_ip::remote_ip(log, headers)
    }

    pub fn save(&self, db_path: &Path, log: &Logger) -> Result<(), RateLimitingPersistError> {
        let snapshot = RateLimitingSnapshot::new(
            self.public.snapshot(),
            self.user.snapshot(),
            self.project.snapshot(),
            self.runner.snapshot(),
            self.bandwidth.snapshot(),
        );
        let snapshot_path = Self::snapshot_path(db_path)?;
        let partial_path = snapshot_path.with_extension("json.partial");
        let json = serde_json::to_string(&snapshot).map_err(RateLimitingPersistError::Serialize)?;
        // Write to a temp file and fsync it before the atomic rename, so a saved snapshot is durable
        // across an abrupt VM stop rather than lingering in the page cache.
        let mut file = std::fs::File::create(&partial_path)
            .map_err(|e| RateLimitingPersistError::Write(e, partial_path.clone()))?;
        file.write_all(json.as_bytes())
            .map_err(|e| RateLimitingPersistError::Write(e, partial_path.clone()))?;
        file.sync_all()
            .map_err(|e| RateLimitingPersistError::Write(e, partial_path.clone()))?;
        drop(file);
        std::fs::rename(&partial_path, &snapshot_path).map_err(|e| {
            RateLimitingPersistError::Rename(e, partial_path, snapshot_path.clone())
        })?;
        // Best-effort fsync of the parent directory so the rename itself is durable. The data file is
        // already fsynced and the rename is atomic, so a directory-sync failure must not fail the save.
        if let Some(parent) = snapshot_path.parent()
            && let Err(e) = std::fs::File::open(parent).and_then(|dir| dir.sync_all())
        {
            slog::debug!(
                log,
                "Non-fatal: failed to fsync snapshot directory {parent}: {e}"
            );
        }
        slog::info!(log, "Saved rate limiting snapshot to {snapshot_path}");
        Ok(())
    }

    pub fn load(&self, db_path: &Path, log: &Logger) -> Result<(), RateLimitingPersistError> {
        let snapshot_path = Self::snapshot_path(db_path)?;
        if !snapshot_path.exists() {
            return Ok(());
        }
        let json = std::fs::read_to_string(&snapshot_path)
            .map_err(|e| RateLimitingPersistError::Read(e, snapshot_path.clone()))?;
        let snapshot: RateLimitingSnapshot = match serde_json::from_str(&json) {
            Ok(snapshot) => snapshot,
            Err(e) => {
                // Log the full raw content so an incompatible/old on-disk format can be diagnosed
                // from the logs (the file is left in place for the next save to overwrite).
                slog::error!(
                    log,
                    "Failed to deserialize rate limiting snapshot from {snapshot_path} ({} bytes)",
                    json.len();
                    "error" => %e,
                    "content" => json.as_str(),
                );
                return Err(RateLimitingPersistError::Deserialize(e));
            },
        };
        let RateLimitingSnapshot {
            public,
            user,
            project,
            runner,
            bandwidth,
        } = snapshot;
        self.public.restore(public);
        self.user.restore(user);
        self.project.restore(project);
        self.runner.restore(runner);
        self.bandwidth.restore(bandwidth);
        slog::info!(log, "Restored rate limiting state from {snapshot_path}");
        Ok(())
    }

    fn snapshot_path(db_path: &Path) -> Result<Utf8PathBuf, RateLimitingPersistError> {
        let db_path = Utf8Path::from_path(db_path).ok_or(RateLimitingPersistError::NonUtf8Path)?;
        let parent = db_path.parent().unwrap_or(Utf8Path::new("."));
        Ok(parent.join("rate_limiting.json"))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RateLimitingPersistError {
    #[error("Database path is not valid UTF-8")]
    NonUtf8Path,
    #[error("Failed to serialize rate limiting snapshot: {0}")]
    Serialize(serde_json::Error),
    #[error("Failed to write rate limiting snapshot to {1}: {0}")]
    Write(std::io::Error, Utf8PathBuf),
    #[error("Failed to rename {1} to {2}: {0}")]
    Rename(std::io::Error, Utf8PathBuf, Utf8PathBuf),
    #[error("Failed to read rate limiting snapshot from {1}: {0}")]
    Read(std::io::Error, Utf8PathBuf),
    #[error("Failed to deserialize rate limiting snapshot: {0}")]
    Deserialize(serde_json::Error),
}

#[cfg(feature = "otel")]
fn interval_kind(interval: bencher_rate_limiter::Interval) -> bencher_otel::IntervalKind {
    match interval {
        bencher_rate_limiter::Interval::Minute => bencher_otel::IntervalKind::Minute,
        bencher_rate_limiter::Interval::Hour => bencher_otel::IntervalKind::Hour,
        bencher_rate_limiter::Interval::Day => bencher_otel::IntervalKind::Day,
    }
}
