use std::{
    io::Write as _,
    net::IpAddr,
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use camino::{Utf8Path, Utf8PathBuf};

use bencher_json::{
    DateTime, Email, OAuthAuthorizationUuid, OAuthGrantUuid, OrganizationUuid, PlanLevel,
    ProjectUuid, RunnerUuid, SessionUuid, UserUuid,
    system::config::{JsonRateLimiting, JsonRateLimits},
};
use bencher_license::Licensor;
#[cfg(feature = "otel")]
use bencher_otel::AuthorizationKind;
use bencher_rate_limiter::{RateLimiter, RateLimits, Reservation};
use dropshot::HttpError;
pub use http::HeaderMap;
use slog::Logger;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::{
    error::{BencherResource, too_many_requests},
    model::{
        organization::{QueryOrganization, plan::LicenseUsage},
        project::{QueryProject, branch::QueryBranch, threshold::QueryThreshold},
    },
};

mod bandwidth;
mod email;
mod ip_key;
mod project;
mod public;
mod remote_ip;
mod runner;
pub(super) mod snapshot;
mod user;

use bandwidth::BandwidthRateLimiter;
use email::{EmailKey, EmailRequesterKey};
use ip_key::IpKey;
use project::ProjectRateLimiter;
use public::PublicRateLimiter;
use runner::RunnerRateLimiter;
use snapshot::RateLimitingSnapshot;
use user::UserRateLimiter;

use super::DbConnection;

/// How often the periodic task compacts the in-memory limiter maps and persists the snapshot.
///
/// This is also the upper bound on how much limiter state an abrupt termination can lose.
const PRUNE_PERIOD: Duration = Duration::from_hours(1);

const DEFAULT_UNCLAIMED_LIMIT: u32 = u8::MAX as u32;
const DEFAULT_CLAIMED_LIMIT: u32 = u16::MAX as u32;
const DEFAULT_PLUS_LIMIT: u32 = 1 << 20;

const DEFAULT_EMAILS_PER_MINUTE_LIMIT: usize = 2;
const DEFAULT_EMAILS_PER_HOUR_LIMIT: usize = 6;
const DEFAULT_EMAILS_PER_DAY_LIMIT: usize = 12;

pub struct RateLimiting {
    // Database-backed rate limits
    window: Duration,
    unclaimed_limit: u32,
    claimed_limit: u32,
    plus_limit: u32,
    // In-memory rate limiters
    public: PublicRateLimiter,
    user: UserRateLimiter,
    project: ProjectRateLimiter,
    runner: RunnerRateLimiter,
    bandwidth: BandwidthRateLimiter,
    /// Refusals of a known OAuth credential, with the public request limits.
    oauth_refusals: RateLimiter<OAuthCredential>,
    /// Sign-in emails to one address from one requester.
    sign_in_email: RateLimiter<EmailRequesterKey>,
    /// Sign-in emails to one address from every requester.
    sign_in_email_ceiling: RateLimiter<EmailKey>,
    /// Fresh sign-in emails one session asks for, from every requester.
    fresh_sign_in_email_ceiling: RateLimiter<SessionUuid>,
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

    #[error("Too many refused requests for OAuth credential per {0}. Please, try again later.")]
    OAuthRefusals(bencher_rate_limiter::Interval),
}

impl Default for RateLimiting {
    fn default() -> Self {
        Self {
            window: bencher_rate_limiter::DAY,
            unclaimed_limit: DEFAULT_UNCLAIMED_LIMIT,
            claimed_limit: DEFAULT_CLAIMED_LIMIT,
            plus_limit: DEFAULT_PLUS_LIMIT,
            public: PublicRateLimiter::default(),
            user: UserRateLimiter::default(),
            project: ProjectRateLimiter::default(),
            runner: RunnerRateLimiter::default(),
            bandwidth: BandwidthRateLimiter::default(),
            oauth_refusals: RateLimiter::new(public::request_limits(None)),
            sign_in_email: RateLimiter::new(email_limits(None)),
            sign_in_email_ceiling: RateLimiter::new(email_ceiling(email_limits(None))),
            fresh_sign_in_email_ceiling: RateLimiter::new(email_ceiling(email_limits(None))),
        }
    }
}

impl From<JsonRateLimiting> for RateLimiting {
    fn from(json: JsonRateLimiting) -> Self {
        let JsonRateLimiting {
            window,
            unclaimed_limit,
            claimed_limit,
            plus_limit,
            public,
            user,
            project,
            runner,
            oci_bandwidth,
            email,
        } = json;
        Self {
            window: window
                .map(u64::from)
                .map_or(bencher_rate_limiter::DAY, Duration::from_secs),
            unclaimed_limit: unclaimed_limit.unwrap_or(DEFAULT_UNCLAIMED_LIMIT),
            claimed_limit: claimed_limit.unwrap_or(DEFAULT_CLAIMED_LIMIT),
            plus_limit: plus_limit.unwrap_or(DEFAULT_PLUS_LIMIT),
            public: public.map_or_else(PublicRateLimiter::default, Into::into),
            user: user.map_or_else(UserRateLimiter::default, Into::into),
            project: project.map_or_else(ProjectRateLimiter::default, Into::into),
            runner: runner.map_or_else(RunnerRateLimiter::default, Into::into),
            bandwidth: oci_bandwidth.map_or_else(BandwidthRateLimiter::default, Into::into),
            oauth_refusals: RateLimiter::new(public::request_limits(public)),
            sign_in_email: RateLimiter::new(email_limits(email)),
            sign_in_email_ceiling: RateLimiter::new(email_ceiling(email_limits(email))),
            fresh_sign_in_email_ceiling: RateLimiter::new(email_ceiling(email_limits(email))),
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
                match LicenseUsage::get_for_server(conn, licensor, Some(PlanLevel::Enterprise)) {
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
            plus_limit: u32::MAX,
            public: PublicRateLimiter::max(),
            user: UserRateLimiter::max(),
            project: ProjectRateLimiter::max(),
            runner: RunnerRateLimiter::max(),
            bandwidth: BandwidthRateLimiter::max(),
            oauth_refusals: RateLimiter::new(RateLimits {
                minute: usize::MAX,
                hour: usize::MAX,
                day: usize::MAX,
            }),
            sign_in_email: RateLimiter::new(RateLimits {
                minute: usize::MAX,
                hour: usize::MAX,
                day: usize::MAX,
            }),
            sign_in_email_ceiling: RateLimiter::new(RateLimits {
                minute: usize::MAX,
                hour: usize::MAX,
                day: usize::MAX,
            }),
            fresh_sign_in_email_ceiling: RateLimiter::new(RateLimits {
                minute: usize::MAX,
                hour: usize::MAX,
                day: usize::MAX,
            }),
        }
    }

    /// Every in-memory limiter wide open, with the database-backed creation limits
    /// set. Exercising a creation ceiling otherwise means throttling the requests
    /// that reach it.
    pub fn max_with_creation_limits(unclaimed_limit: u32, claimed_limit: u32) -> Self {
        Self::max_with_creation_and_plus_limits(unclaimed_limit, claimed_limit, u32::MAX)
    }

    /// [`Self::max_with_creation_limits`] with the daily metrics limit of a paid
    /// organization set too.
    pub fn max_with_creation_and_plus_limits(
        unclaimed_limit: u32,
        claimed_limit: u32,
        plus_limit: u32,
    ) -> Self {
        Self {
            unclaimed_limit,
            claimed_limit,
            plus_limit,
            ..Self::max()
        }
    }

    /// Compact every in-memory limiter map, dropping the keys whose events have all aged out.
    ///
    /// Returns the total number of evicted entries.
    pub fn prune(&self) -> usize {
        self.public.prune()
            + self.user.prune()
            + self.project.prune()
            + self.runner.prune()
            + self.bandwidth.prune()
            + self.oauth_refusals.prune()
            + self.sign_in_email.prune()
            + self.sign_in_email_ceiling.prune()
            + self.fresh_sign_in_email_ceiling.prune()
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
        self.check_claimable_limit_with_count(
            is_claimed,
            window_usage,
            NonZeroU32::MIN,
            unclaimed_error_fn,
            claimed_error_fn,
        )
    }

    pub fn check_claimable_limit_with_count<FUn, FCl>(
        &self,
        is_claimed: bool,
        window_usage: u32,
        count: NonZeroU32,
        unclaimed_error_fn: FUn,
        claimed_error_fn: FCl,
    ) -> Result<(), HttpError>
    where
        FUn: FnOnce(u32) -> RateLimitingError,
        FCl: FnOnce(u32) -> RateLimitingError,
    {
        if is_claimed {
            Self::check_inner(
                self.claimed_limit,
                window_usage,
                count,
                claimed_error_fn,
                #[cfg(feature = "otel")]
                AuthorizationKind::User,
            )
        } else {
            Self::check_inner(
                self.unclaimed_limit,
                window_usage,
                count,
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
            NonZeroU32::MIN,
            error_fn,
            #[cfg(feature = "otel")]
            AuthorizationKind::User,
        )
    }

    pub fn check_plus_limit<F>(&self, window_usage: u32, error_fn: F) -> Result<(), HttpError>
    where
        F: FnOnce(u32) -> RateLimitingError,
    {
        Self::check_inner(
            self.plus_limit,
            window_usage,
            NonZeroU32::MIN,
            error_fn,
            #[cfg(feature = "otel")]
            AuthorizationKind::User,
        )
    }

    fn check_inner<F>(
        limit: u32,
        window_usage: u32,
        count: NonZeroU32,
        error_fn: F,
        #[cfg(feature = "otel")] authorization_kind: AuthorizationKind,
    ) -> Result<(), HttpError>
    where
        F: FnOnce(u32) -> RateLimitingError,
    {
        if window_usage
            .checked_add(count.get())
            .is_some_and(|usage| usage <= limit)
        {
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

    /// Charge a refusal of a known OAuth credential to that credential alone, never to its user,
    /// so that whoever holds a dead credential cannot spend its user's limits.
    pub fn oauth_refusal(&self, credential: OAuthCredential) -> Result<(), HttpError> {
        if let Some(interval) = self.oauth_refusals.check(credential) {
            Err(too_many_requests(RateLimitingError::OAuthRefusals(
                interval,
            )))
        } else {
            Ok(())
        }
    }

    /// Hold a sign-in email to `email`, asked for by `requester`, against its caps: the tight caps
    /// on what one requester may send the address, and the day's ceiling the email is held to,
    /// on what every requester together may send. A request with no known address counts as one
    /// requester with every other such request. At a cap, hold nothing and return how long until
    /// every cap that applies has room.
    pub fn sign_in_email(
        &self,
        email: &Email,
        requester: Option<IpAddr>,
        ceiling: SignInEmailCeiling,
    ) -> Result<SignInEmailHold, Duration> {
        let address = EmailKey::from(email);
        let key = EmailRequesterKey::new(address, requester);
        let reserved = match self.sign_in_email.reserve(key) {
            Ok(reserved) => reserved,
            Err(wait) => {
                let ceiling_wait = match ceiling {
                    SignInEmailCeiling::Address => self.sign_in_email_ceiling.wait(address),
                    SignInEmailCeiling::Session(session) => {
                        self.fresh_sign_in_email_ceiling.wait(session)
                    },
                };
                return Err(ceiling_wait.map_or(wait, |ceiling_wait| wait.max(ceiling_wait)));
            },
        };
        let ceiling = match ceiling {
            SignInEmailCeiling::Address => self
                .sign_in_email_ceiling
                .reserve(address)
                .map(|reserved| CeilingHold::Address(address, reserved)),
            SignInEmailCeiling::Session(session) => self
                .fresh_sign_in_email_ceiling
                .reserve(session)
                .map(|reserved| CeilingHold::Session(session, reserved)),
        };
        match ceiling {
            Ok(ceiling) => Ok(SignInEmailHold {
                requester: (key, reserved),
                ceiling,
            }),
            Err(wait) => {
                self.sign_in_email.release(key, reserved);
                Err(wait)
            },
        }
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

    pub(super) fn remote_ip(log: &Logger, headers: &HeaderMap, relayed: bool) -> Option<IpAddr> {
        remote_ip::remote_ip(log, headers, relayed)
    }

    pub fn save(&self, db_path: &Path, log: &Logger) -> Result<(), RateLimitingPersistError> {
        let snapshot = RateLimitingSnapshot {
            public: self.public.snapshot(),
            user: self.user.snapshot(),
            project: self.project.snapshot(),
            runner: self.runner.snapshot(),
            bandwidth: self.bandwidth.snapshot(),
            sign_in_email: self.sign_in_email.snapshot(),
            sign_in_email_ceiling: self.sign_in_email_ceiling.snapshot(),
            fresh_sign_in_email_ceiling: self.fresh_sign_in_email_ceiling.snapshot(),
        };
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
            sign_in_email,
            sign_in_email_ceiling,
            fresh_sign_in_email_ceiling,
        } = snapshot;
        self.public.restore(public);
        self.user.restore(user);
        self.project.restore(project);
        self.runner.restore(runner);
        self.bandwidth.restore(bandwidth);
        self.sign_in_email.restore(sign_in_email);
        self.sign_in_email_ceiling.restore(sign_in_email_ceiling);
        self.fresh_sign_in_email_ceiling
            .restore(fresh_sign_in_email_ceiling);
        slog::info!(log, "Restored rate limiting state from {snapshot_path}");
        Ok(())
    }

    /// Spawn the periodic task that compacts the in-memory limiter maps and persists the snapshot.
    ///
    /// Without it, a long-lived server only ever compacts at a clean shutdown, so its limiter maps
    /// grow for as long as the process lives, and an abrupt termination loses all limiter state.
    ///
    /// The returned handle lets shutdown wait for an in-flight save to finish before it writes the
    /// final snapshot, so the two never race over the same partial file.
    pub fn spawn_prune(
        rate_limiting: Arc<Self>,
        db_path: PathBuf,
        log: Logger,
        shutdown: CancellationToken,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            // The first tick of an `interval` fires immediately. Start a full period out instead:
            // the snapshot was just loaded, so there is nothing to compact or persist at startup.
            let mut interval =
                tokio::time::interval_at(tokio::time::Instant::now() + PRUNE_PERIOD, PRUNE_PERIOD);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            while shutdown
                .run_until_cancelled(interval.tick())
                .await
                .is_some()
            {
                rate_limiting.prune_and_save(&db_path, &log);
            }
        })
    }

    /// Compact the in-memory limiter maps and persist the resulting snapshot.
    ///
    /// Shared by the periodic task and the shutdown path, which are serialized against each other:
    /// shutdown awaits the periodic task before it runs its own final pass.
    ///
    /// Returns the number of evicted entries. Both the count and a failed save are logged here, and
    /// the failure is swallowed so that a transient write error cannot kill the periodic task.
    pub fn prune_and_save(&self, db_path: &Path, log: &Logger) -> usize {
        let evicted = self.prune();
        if evicted > 0 {
            slog::info!(log, "Pruned {evicted} stale rate limiting entries");
        }
        if let Err(e) = self.save(db_path, log) {
            slog::error!(log, "Failed to save rate limiting state: {e}");
        }
        evicted
    }

    fn snapshot_path(db_path: &Path) -> Result<Utf8PathBuf, RateLimitingPersistError> {
        let db_path = Utf8Path::from_path(db_path).ok_or(RateLimitingPersistError::NonUtf8Path)?;
        let parent = db_path.parent().unwrap_or(Utf8Path::new("."));
        Ok(parent.join("rate_limiting.json"))
    }
}

/// The day's ceiling a sign-in email is held to, on what every requester together may send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignInEmailCeiling {
    /// The address's, as every sign-in email is held to but a fresh sign-in's.
    Address,
    /// The session's that asks, as a fresh sign-in's email is held to, since a live session of
    /// the address's user asks for it.
    Session(SessionUuid),
}

/// A sign-in email held against its caps, to give back should it not be sent.
#[derive(Debug)]
#[must_use]
pub struct SignInEmailHold {
    requester: (EmailRequesterKey, Reservation),
    ceiling: CeilingHold,
}

/// A sign-in email held against the day's ceiling it is held to.
#[derive(Debug)]
enum CeilingHold {
    Address(EmailKey, Reservation),
    Session(SessionUuid, Reservation),
}

impl SignInEmailHold {
    /// Give the email back to the caps it was held against, since it was never sent.
    pub fn release(self, rate_limiting: &RateLimiting) {
        let Self {
            requester: (key, reserved),
            ceiling,
        } = self;
        rate_limiting.sign_in_email.release(key, reserved);
        match ceiling {
            CeilingHold::Address(address, reserved) => rate_limiting
                .sign_in_email_ceiling
                .release(address, reserved),
            CeilingHold::Session(session, reserved) => rate_limiting
                .fresh_sign_in_email_ceiling
                .release(session, reserved),
        }
    }
}

/// A known OAuth credential, by what it belongs to: a grant for its tokens, or a pending
/// authorization for its code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OAuthCredential {
    Grant(OAuthGrantUuid),
    Authorization(OAuthAuthorizationUuid),
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

/// Holds the periodic prune task's join handle so that shutdown can wait for it.
///
/// Shutdown only ever has a shared reference to the API context, hence the interior mutability.
#[derive(Default)]
pub struct RateLimitingPruneTask(Mutex<Option<JoinHandle<()>>>);

impl RateLimitingPruneTask {
    pub fn new(handle: JoinHandle<()>) -> Self {
        Self(Mutex::new(Some(handle)))
    }

    /// Take the join handle, leaving the slot empty.
    ///
    /// Returns `None` if the handle was already taken or the lock was poisoned. Neither is worth
    /// failing shutdown over: the task stops on its own once the shutdown signal is tripped.
    pub fn take(&self) -> Option<JoinHandle<()>> {
        self.0.lock().ok().and_then(|mut handle| handle.take())
    }
}

/// The limits on sign-in emails to one address from one requester, as configured.
fn email_limits(email: Option<JsonRateLimits>) -> RateLimits {
    RateLimits {
        minute: email
            .and_then(|r| r.minute)
            .unwrap_or(DEFAULT_EMAILS_PER_MINUTE_LIMIT),
        hour: email
            .and_then(|r| r.hour)
            .unwrap_or(DEFAULT_EMAILS_PER_HOUR_LIMIT),
        day: email
            .and_then(|r| r.day)
            .unwrap_or(DEFAULT_EMAILS_PER_DAY_LIMIT),
    }
}

/// The limit on the sign-in emails held to one day's ceiling, from every requester: twice what
/// one requester may have sent to one address in a day.
fn email_ceiling(limits: RateLimits) -> RateLimits {
    RateLimits {
        minute: usize::MAX,
        hour: usize::MAX,
        day: limits.day.saturating_mul(2),
    }
}

#[cfg(feature = "otel")]
fn interval_kind(interval: bencher_rate_limiter::Interval) -> bencher_otel::IntervalKind {
    match interval {
        bencher_rate_limiter::Interval::Minute => bencher_otel::IntervalKind::Minute,
        bencher_rate_limiter::Interval::Hour => bencher_otel::IntervalKind::Hour,
        bencher_rate_limiter::Interval::Day => bencher_otel::IntervalKind::Day,
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use bencher_json::{
        Email, OrganizationUuid, PlanLevel, Secret, SessionUuid, system::config::JsonRateLimiting,
    };
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    use super::{
        Arc, CancellationToken, IpKey, Licensor, Logger, RateLimiting, RateLimitingPruneTask,
        RateLimitingSnapshot, SignInEmailCeiling, SignInEmailHold,
    };
    use crate::{
        schema,
        test_util::{create_base_entities, setup_test_db},
    };

    // Debug builds trust this key's public half for Self-Hosted licenses.
    const TEST_LICENSE_KEY: &str =
        include_str!("../../../../../plus/bencher_license/src/test/private.pem");
    const CUSTOM_PLUS_LIMIT: u32 = 7;

    fn test_log() -> Logger {
        Logger::root(slog::Discard, slog::o!())
    }

    // Each day's ceiling, on one address and on one session, is twice the daily cap on one
    // requester: 24 at the defaults, 100 with a day of 50, and none where no limits apply.
    // Kills a ceiling of a fixed size, or one where no limits apply.
    #[test]
    fn sign_in_email_ceiling_is_twice_the_day_limit() {
        let day_of_50: JsonRateLimiting = serde_json::from_value(serde_json::json!({
            "email": {"minute": 50, "hour": 50, "day": 50}
        }))
        .unwrap();
        for ceiling in [
            SignInEmailCeiling::Address,
            SignInEmailCeiling::Session(SessionUuid::new()),
        ] {
            assert_eq!(holds(&RateLimiting::default(), ceiling, 1, 1000).len(), 24);
            let day_of_50 = RateLimiting::from(day_of_50);
            assert_eq!(holds(&day_of_50, ceiling, 1, 1000).len(), 100);
            assert_eq!(holds(&RateLimiting::max(), ceiling, 1, 1000).len(), 1000);
        }
    }

    // Holds given back, as for sends that failed, give back the day's ceiling with each
    // requester's share: the full ceiling, from the same requesters, holds again.
    // Kills a hold given back to its requester's caps but not to the ceiling.
    #[test]
    fn released_holds_give_back_the_ceiling() {
        for ceiling in [
            SignInEmailCeiling::Address,
            SignInEmailCeiling::Session(SessionUuid::new()),
        ] {
            let rate_limiting = RateLimiting::default();
            let held = holds(&rate_limiting, ceiling, 1, 1000);
            assert_eq!(held.len(), 24);
            for hold in held {
                hold.release(&rate_limiting);
            }
            assert_eq!(holds(&rate_limiting, ceiling, 1, 1000).len(), 24);
        }
    }

    // A session's fresh sign-in emails stop at its own ceiling, from every requester together,
    // which neither the address's ceiling nor another session's spends: with one session at its
    // ceiling, another of the same address still has the whole of its own, and the address
    // still has the whole of its.
    // Kills a fresh sign-in email past its session's ceiling, one ceiling for every session or
    // for the address, or a fresh sign-in email charged to the address's ceiling.
    #[test]
    fn a_sessions_ceiling_is_its_own() {
        let rate_limiting = RateLimiting::default();
        let session = SignInEmailCeiling::Session(SessionUuid::new());
        assert_eq!(holds(&rate_limiting, session, 1, 1000).len(), 24);
        assert_eq!(holds(&rate_limiting, session, 101, 1000).len(), 0);
        let other = SignInEmailCeiling::Session(SessionUuid::new());
        assert_eq!(holds(&rate_limiting, other, 201, 1000).len(), 24);
        let address = SignInEmailCeiling::Address;
        assert_eq!(holds(&rate_limiting, address, 301, 1000).len(), 24);
    }

    /// The sign-in emails to one address held to `ceiling`, from requesters counted up from
    /// `first` that each ask until refused, a new requester at a time, until a new one is refused
    /// at once, or until `most`.
    fn holds(
        rate_limiting: &RateLimiting,
        ceiling: SignInEmailCeiling,
        first: u32,
        most: usize,
    ) -> Vec<SignInEmailHold> {
        let email: Email = "ada@example.com".parse().unwrap();
        let mut held = Vec::new();
        for requester in first..=u32::from(u16::MAX) {
            let requester = IpAddr::from(Ipv4Addr::from(0x0a00_0000 + requester));
            let before = held.len();
            while let Ok(hold) = rate_limiting.sign_in_email(&email, Some(requester), ceiling) {
                held.push(hold);
                if held.len() == most {
                    return held;
                }
            }
            if held.len() == before {
                break;
            }
        }
        held
    }

    /// The daily metrics limit a Self-Hosted server applies when it is configured
    /// with custom rate limits and its one organization holds a license at `level`.
    fn self_hosted_plus_limit(level: PlanLevel) -> u32 {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let organization: OrganizationUuid = schema::organization::table
            .find(base.organization_id)
            .select(schema::organization::uuid)
            .first(&mut conn)
            .unwrap();
        let key: Secret = TEST_LICENSE_KEY.parse().unwrap();
        let license = Licensor::bencher_cloud(&key)
            .unwrap()
            .new_annual_license(organization, level, 1_000.try_into().unwrap())
            .unwrap();
        diesel::update(schema::organization::table.find(base.organization_id))
            .set(schema::organization::license.eq(license))
            .execute(&mut conn)
            .unwrap();

        let custom = JsonRateLimiting {
            window: None,
            unclaimed_limit: None,
            claimed_limit: None,
            plus_limit: Some(CUSTOM_PLUS_LIMIT),
            public: None,
            user: None,
            project: None,
            runner: None,
            oci_bandwidth: None,
            email: None,
        };
        RateLimiting::new(
            &test_log(),
            &mut conn,
            &Licensor::self_hosted().unwrap(),
            false,
            Some(custom),
        )
        .plus_limit
    }

    #[test]
    fn enterprise_license_enables_custom_rate_limits() {
        assert_eq!(
            self_hosted_plus_limit(PlanLevel::Enterprise),
            CUSTOM_PLUS_LIMIT
        );
    }

    #[test]
    fn free_license_keeps_rate_limits_off() {
        assert_eq!(self_hosted_plus_limit(PlanLevel::Free), u32::MAX);
    }

    #[test]
    fn pro_license_keeps_rate_limits_off() {
        assert_eq!(self_hosted_plus_limit(PlanLevel::Pro), u32::MAX);
    }

    #[test]
    fn prune_and_save_persists_live_entries() {
        let rate_limiting = RateLimiting::default();
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        rate_limiting.public_request(ip).unwrap();

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("bencher.db");
        let log = test_log();

        // Every seeded entry is live, so nothing is evicted.
        assert_eq!(rate_limiting.prune_and_save(&db_path, &log), 0);

        let snapshot_path = dir.path().join("rate_limiting.json");
        assert!(snapshot_path.exists());

        let restored = RateLimiting::default();
        restored.load(&db_path, &log).unwrap();
        assert_eq!(restored.prune_and_save(&db_path, &log), 0);

        let json = std::fs::read_to_string(&snapshot_path).unwrap();
        let snapshot: RateLimitingSnapshot = serde_json::from_str(&json).unwrap();
        assert!(
            snapshot
                .public
                .requests
                .minute
                .events
                .contains_key(&IpKey::from(ip))
        );
    }

    #[tokio::test]
    async fn spawn_prune_stops_on_the_shutdown_signal() {
        let dir = tempfile::tempdir().unwrap();
        let shutdown = CancellationToken::new();
        let handle = RateLimiting::spawn_prune(
            Arc::new(RateLimiting::default()),
            dir.path().join("bencher.db"),
            test_log(),
            shutdown.clone(),
        );

        // The first tick is a full period out, so the task is still waiting on it.
        assert!(!handle.is_finished());

        shutdown.cancel();
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn prune_task_slot_yields_the_handle_once() {
        let handle = tokio::spawn(async {});
        let slot = RateLimitingPruneTask::new(handle);

        slot.take().unwrap().await.unwrap();
        assert!(slot.take().is_none());
    }

    #[test]
    fn prune_and_save_survives_a_failed_save() {
        let rate_limiting = RateLimiting::default();
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        rate_limiting.public_request(ip).unwrap();

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("missing").join("bencher.db");

        assert_eq!(rate_limiting.prune_and_save(&db_path, &test_log()), 0);
        assert!(!dir.path().join("missing").exists());
    }
}
