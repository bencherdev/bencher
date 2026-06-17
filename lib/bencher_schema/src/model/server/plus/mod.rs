#![cfg(feature = "plus")]

use std::cmp;
use std::path::PathBuf;
use std::sync::LazyLock;

use bencher_billing::Biller;
use bencher_json::system::server::SelfHostedStats;
use bencher_json::{
    BENCHER_API_URL, BENCHER_API_VERSION, BooleanParam, DateTime, JsonServer, JsonServerStats,
    PlanLevel, SelfHostedStartup, ServerUuid, organization::plan::PRO_INCLUDED_CREDIT_CENTS,
};
use bencher_license::Licensor;
use chrono::{Duration, NaiveTime, Utc};
use diesel::{Connection as _, RunQueryDsl as _, connection::SimpleConnection as _};
use dropshot::HttpError;
use slog::Logger;
use url::Url;

use crate::resource_not_found_err;
use crate::{
    context::StatsSettings,
    context::{Body, DbConnection, Message, Messenger, ServerStatsBody},
    error::{request_timeout_error, resource_conflict_err},
    macros::fn_get::fn_get,
    model::{
        organization::plan::{LicenseUsage, QueryPlan},
        user::QueryUser,
    },
    schema::{self, server as server_table},
};

mod stats;

crate::macros::typed_id::typed_id!(ServerId);

const SERVER_ID: ServerId = ServerId(1);

const LICENSE_GRACE_PERIOD: usize = 7;

/// Time of day (UTC) at which the daily Pro credit sweep runs. A fixed time so
/// the sweep happens at the same time every day rather than drifting with
/// server restarts.
const CREDIT_SWEEP_TIME: NaiveTime = NaiveTime::MIN;

/// How long to wait before retrying a failed database connection during the
/// daily credit sweep, so a transient failure does not skip the whole day's run.
const CREDIT_SWEEP_RETRY_DELAY: std::time::Duration = std::time::Duration::from_mins(1);

/// Maximum number of connection attempts for a single daily sweep before giving
/// up until the next scheduled run.
const CREDIT_SWEEP_RETRY_LIMIT: usize = 5;

#[expect(clippy::panic, reason = "valid constant URL with known path")]
static BENCHER_STATS_API_URL: LazyLock<Url> = LazyLock::new(|| {
    BENCHER_API_URL
        .clone()
        .join("/v0/server/stats")
        .unwrap_or_else(|e| panic!("Failed to parse stats API endpoint: {e}"))
});

#[derive(Debug, Clone, Copy, diesel::Queryable)]
pub struct QueryServer {
    pub id: ServerId,
    pub uuid: ServerUuid,
    pub created: DateTime,
}

impl QueryServer {
    fn_get!(server, ServerId);

    pub fn get_server(conn: &mut DbConnection) -> Result<Self, HttpError> {
        Self::get(conn, SERVER_ID)
    }

    pub fn get_or_create(conn: &mut DbConnection) -> Result<Self, HttpError> {
        if let Ok(server) = Self::get_server(conn) {
            Ok(server)
        } else {
            let server = InsertServer::default();
            diesel::insert_into(schema::server::table)
                .values(&server)
                .execute(conn)
                .map_err(resource_conflict_err!(Server, SERVER_ID))?;
            Self::get_server(conn)
        }
    }

    #[expect(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "refactor stats handling"
    )]
    pub fn spawn_stats(
        self,
        log: Logger,
        db_path: PathBuf,
        busy_timeout: u32,
        stats: StatsSettings,
        messenger: Option<Messenger>,
        licensor: Licensor,
        is_bencher_cloud: bool,
    ) -> Result<(), HttpError> {
        let server_stats_url = self
            .server_stats_url()
            .map_err(resource_not_found_err!(Server, &db_path))?;
        slog::info!(log, "Server stats endpoint: {server_stats_url}");

        tokio::spawn(async move {
            let StatsSettings { offset, enabled } = stats;

            if !is_bencher_cloud {
                let client = reqwest::Client::new();
                if let Err(e) = client
                    .get(server_stats_url.clone())
                    .query(&BooleanParam::True(SelfHostedStartup))
                    .send()
                    .await
                {
                    slog::warn!(log, "Failed to register startup: {e}");
                }
            }

            let mut violations = 0;
            loop {
                let now = Utc::now().naive_utc().time();
                let sleep_time = match now.cmp(&offset) {
                    cmp::Ordering::Less => offset - now,
                    cmp::Ordering::Equal => Duration::days(1),
                    cmp::Ordering::Greater => Duration::days(1) - (now - offset),
                }
                .to_std()
                .unwrap_or(std::time::Duration::from_hours(24));
                tokio::time::sleep(sleep_time).await;

                let Ok(mut conn) = DbConnection::establish(db_path.to_string_lossy().as_ref())
                else {
                    slog::error!(log, "Failed to establish database connection");
                    continue;
                };

                if let Err(e) = configure_standalone_connection(&mut conn, busy_timeout) {
                    slog::error!(log, "Failed to configure database connection PRAGMAs: {e}");
                    continue;
                }

                if enabled {
                    slog::info!(log, "Sending stats at {}", Utc::now());
                } else if is_bencher_cloud {
                    slog::info!(
                        log,
                        "Sending stats is disabled, but running on Bencher Cloud"
                    );
                } else {
                    match LicenseUsage::get_for_server(&mut conn, &licensor, Some(PlanLevel::Team))
                    {
                        Ok(license_usages) if license_usages.is_empty() => {
                            violations += 1;
                            // Be kind. Allow for a seven day grace period.
                            slog::warn!(
                                log,
                                "Sending stats is disabled, but there is no valid Bencher Plus license key! This is violation #{violations} of the Bencher License: https://bencher.dev/legal/license"
                            );
                            if let Some(remaining) = LICENSE_GRACE_PERIOD.checked_sub(violations) {
                                slog::warn!(
                                    log,
                                    "You have {remaining} days remaining in your Bencher License grace period. Please purchase a license key: https://bencher.dev/pricing"
                                );
                                continue;
                            }
                            slog::warn!(
                                log,
                                "Sending stats at {}. Please purchase a license key: https://bencher.dev/pricing",
                                Utc::now()
                            );
                        },
                        Ok(_) => {
                            slog::debug!(log, "Sending stats is disabled");
                            continue;
                        },
                        Err(e) => {
                            slog::error!(log, "Failed to check stats: {e}");
                            continue;
                        },
                    }
                }

                if !is_bencher_cloud {
                    let client = reqwest::Client::new();
                    if let Err(e) = client
                        .get(server_stats_url.clone())
                        .query(&BooleanParam::True(SelfHostedStats))
                        .send()
                        .await
                    {
                        slog::warn!(log, "Failed to register stats: {e}");
                    }
                }

                let Some(json_stats_str) = self.get_stats_str(&log, conn, is_bencher_cloud).await
                else {
                    slog::error!(log, "Failed to get stats string");
                    continue;
                };

                if let Some(messenger) = messenger.as_ref() {
                    slog::info!(log, "Bencher Cloud Stats: {json_stats_str:?}");

                    let Ok(mut conn) = DbConnection::establish(db_path.to_string_lossy().as_ref())
                    else {
                        slog::error!(
                            log,
                            "Failed to establish database connection for sending stats"
                        );
                        continue;
                    };

                    if let Err(e) = configure_standalone_connection(&mut conn, busy_timeout) {
                        slog::error!(
                            log,
                            "Failed to configure database connection PRAGMAs for sending stats: {e}"
                        );
                        continue;
                    }

                    if let Err(e) = Self::send_stats_to_backend(
                        &log,
                        &mut conn,
                        messenger,
                        &json_stats_str,
                        None,
                    ) {
                        slog::error!(log, "Failed to send stats: {e}");
                    }
                } else {
                    let client = reqwest::Client::new();
                    if let Err(e) = client
                        .post(BENCHER_STATS_API_URL.clone())
                        .body(json_stats_str)
                        .send()
                        .await
                    {
                        slog::error!(log, "Failed to send stats: {e}");
                    }
                }
            }
        });

        Ok(())
    }

    pub fn server_stats_url(&self) -> Result<Url, url::ParseError> {
        format!(
            "{url}/{uuid}",
            url = BENCHER_STATS_API_URL.clone(),
            uuid = self.uuid
        )
        .parse()
    }

    pub async fn get_stats(
        self,
        log: Logger,
        mut conn: DbConnection,
        is_bencher_cloud: bool,
    ) -> Result<JsonServerStats, HttpError> {
        tokio::task::spawn_blocking(move || {
            stats::get_stats(&log, &mut conn, self, is_bencher_cloud)
        })
        .await
        .map_err(request_timeout_error)?
    }

    async fn get_stats_str(
        self,
        log: &Logger,
        conn: DbConnection,
        is_bencher_cloud: bool,
    ) -> Option<String> {
        let json_stats = self
            .get_stats(log.clone(), conn, is_bencher_cloud)
            .await
            .inspect_err(|e| {
                slog::error!(log, "Failed to get stats: {e}");
            })
            .ok()?;

        serde_json::to_string_pretty(&json_stats)
            .inspect_err(|e| {
                slog::error!(log, "Failed to serialize stats: {e}");
            })
            .ok()
    }

    pub fn send_stats_to_backend(
        log: &Logger,
        conn: &mut DbConnection,
        messenger: &Messenger,
        server_stats: &str,
        self_hosted_server: Option<ServerUuid>,
    ) -> Result<(), HttpError> {
        // TODO find a better home for these than my inbox
        let admins = QueryUser::get_admins(conn)?;

        for admin in admins {
            let message = Message {
                to_name: Some(admin.name.clone().into()),
                to_email: admin.email.into(),
                subject: Some(if let Some(server) = self_hosted_server {
                    format!("🐰 Self-Hosted Server Stats ({server})")
                } else {
                    "🐰 Bencher Cloud Server Stats".to_owned()
                }),
                body: Some(Body::ServerStats(ServerStatsBody {
                    server_stats: server_stats.to_owned(),
                })),
            };
            messenger.send(log, message);
        }
        Ok(())
    }

    pub fn into_json(self) -> JsonServer {
        let Self { uuid, created, .. } = self;
        JsonServer {
            uuid,
            created,
            version: Some(BENCHER_API_VERSION.into()),
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = server_table)]
pub struct InsertServer {
    pub uuid: ServerUuid,
    pub created: DateTime,
}

impl Default for InsertServer {
    fn default() -> Self {
        Self {
            uuid: ServerUuid::new(),
            created: DateTime::now(),
        }
    }
}

/// Daily background sweep (Bencher Cloud only) that keeps each Pro
/// subscription's included usage credit granted for the current billing period
/// and prunes the local plan row once a subscription has fully lapsed. This
/// replaces granting credit on the report-ingestion path, keeping that hot path
/// free of billing-side Stripe calls.
pub fn spawn_pro_credit_grants(log: Logger, db_path: PathBuf, busy_timeout: u32, biller: Biller) {
    tokio::spawn(async move {
        loop {
            // Sleep until the next occurrence of the daily sweep time (UTC) so the
            // sweep runs at a fixed time of day, not relative to server start.
            let now = Utc::now().naive_utc().time();
            let sleep_time = match now.cmp(&CREDIT_SWEEP_TIME) {
                cmp::Ordering::Less => CREDIT_SWEEP_TIME - now,
                cmp::Ordering::Equal => Duration::days(1),
                cmp::Ordering::Greater => Duration::days(1) - (now - CREDIT_SWEEP_TIME),
            }
            .to_std()
            .unwrap_or(std::time::Duration::from_hours(24));
            tokio::time::sleep(sleep_time).await;

            // Open a configured connection for this sweep, retrying briefly on a
            // transient failure rather than skipping the whole day's run.
            let mut attempt = 0;
            let conn = loop {
                attempt += 1;
                match DbConnection::establish(db_path.to_string_lossy().as_ref()) {
                    Ok(mut conn) => {
                        match configure_standalone_connection(&mut conn, busy_timeout) {
                            Ok(()) => break Some(conn),
                            Err(e) => slog::error!(
                                log,
                                "Failed to configure database connection PRAGMAs for credit sweep: {e}"
                            ),
                        }
                    },
                    Err(e) => slog::error!(
                        log,
                        "Failed to establish database connection for credit sweep: {e}"
                    ),
                }
                if attempt >= CREDIT_SWEEP_RETRY_LIMIT {
                    break None;
                }
                tokio::time::sleep(CREDIT_SWEEP_RETRY_DELAY).await;
            };
            let Some(mut conn) = conn else {
                slog::error!(
                    log,
                    "Giving up on credit sweep until the next scheduled run after {CREDIT_SWEEP_RETRY_LIMIT} failed connection attempts"
                );
                continue;
            };

            let plans = match QueryPlan::all_metered(&mut conn) {
                Ok(plans) => plans,
                Err(e) => {
                    slog::error!(log, "Failed to load metered plans for credit sweep: {e}");
                    continue;
                },
            };

            for plan in plans {
                let Some(metered_plan_id) = plan.metered_plan.clone() else {
                    continue;
                };
                let plan_id = plan.id;
                match biller.get_metered_plan_status(&metered_plan_id).await {
                    // Active (or trialing): ensure this period's included credit.
                    // Idempotent, and a no-op for grandfathered Team metered plans.
                    Ok((status, _)) if status.is_active() => {
                        if let Err(e) = biller
                            .ensure_period_credit(&metered_plan_id, PRO_INCLUDED_CREDIT_CENTS)
                            .await
                        {
                            slog::warn!(
                                log,
                                "Failed to ensure period credit for {metered_plan_id}: {e}"
                            );
                            #[cfg(feature = "sentry")]
                            sentry::capture_error(&e);
                        }
                    },
                    // Lapsed (canceled/past due/unpaid): prune the local plan row so
                    // the org reads as Free and we stop querying a dead subscription.
                    Ok(_) => {
                        if let Err(e) = QueryPlan::delete(&mut conn, plan_id) {
                            slog::warn!(log, "Failed to prune lapsed plan {metered_plan_id}: {e}");
                        }
                    },
                    Err(e) => {
                        slog::warn!(log, "Failed to fetch status for {metered_plan_id}: {e}");
                        #[cfg(feature = "sentry")]
                        sentry::capture_error(&e);
                    },
                }
            }
        }
    });
}

fn configure_standalone_connection(
    conn: &mut DbConnection,
    busy_timeout: u32,
) -> diesel::QueryResult<()> {
    conn.batch_execute(&format!("PRAGMA busy_timeout = {busy_timeout}"))?;
    conn.batch_execute("PRAGMA synchronous = NORMAL")?;
    conn.batch_execute("PRAGMA extended_result_codes = ON")?;
    Ok(())
}
