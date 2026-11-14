#![cfg(feature = "plus")]

use std::cmp;
use std::path::PathBuf;
use std::sync::LazyLock;

use bencher_json::system::server::SelfHostedStats;
use bencher_json::{
    BENCHER_API_URL, BENCHER_API_VERSION, BooleanParam, DateTime, JsonServer, JsonServerStats,
    SelfHostedStartup, ServerUuid,
};
use bencher_license::Licensor;
use chrono::{Duration, Utc};
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
    model::{organization::license::any_valid_license, user::QueryUser},
    schema::{self, server as server_table},
};

mod stats;

crate::macros::typed_id::typed_id!(ServerId);

const SERVER_ID: ServerId = ServerId(1);

const LICENSE_GRACE_PERIOD: usize = 7;

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

            let mut grace = LicenseGrace::default();
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
                    let licensed = match any_valid_license(&mut conn, &licensor, DateTime::now()) {
                        Ok(licensed) => licensed,
                        Err(e) => {
                            slog::error!(log, "Failed to check stats: {e}");
                            continue;
                        },
                    };
                    match grace.check(licensed) {
                        OptOut::Licensed => {
                            slog::debug!(log, "Sending stats is disabled");
                            continue;
                        },
                        OptOut::Grace {
                            violations,
                            remaining,
                        } => {
                            warn_violation(&log, violations);
                            slog::warn!(
                                log,
                                "You have {remaining} days remaining in your Bencher License grace period. Please purchase a license key: https://bencher.dev/pricing"
                            );
                            continue;
                        },
                        OptOut::Send { violations } => {
                            warn_violation(&log, violations);
                            slog::warn!(
                                log,
                                "Sending stats at {}. Please purchase a license key: https://bencher.dev/pricing",
                                Utc::now()
                            );
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

/// What the daily stats loop does on a Self-Hosted server that has turned sending them off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OptOut {
    /// A valid license allows it: skip the stats, and the grace starts again from the beginning.
    Licensed,
    /// No valid license: count a violation, and skip the stats while days of grace remain.
    Grace { violations: usize, remaining: usize },
    /// No valid license and no grace left: count a violation, and send the stats.
    Send { violations: usize },
}

/// The grace the daily stats loop gives a Self-Hosted server that has turned sending them off.
#[derive(Debug, Default)]
struct LicenseGrace {
    /// The days without a valid license since the last day with one.
    violations: usize,
}

impl LicenseGrace {
    /// Check today's license: a valid one starts the grace again from the beginning, and
    /// without one the day counts as a violation.
    fn check(&mut self, licensed: bool) -> OptOut {
        if licensed {
            self.violations = 0;
            return OptOut::Licensed;
        }
        self.violations = self.violations.saturating_add(1);
        // Be kind. Allow for a seven day grace period.
        if let Some(remaining) = LICENSE_GRACE_PERIOD.checked_sub(self.violations) {
            OptOut::Grace {
                violations: self.violations,
                remaining,
            }
        } else {
            OptOut::Send {
                violations: self.violations,
            }
        }
    }
}

fn warn_violation(log: &Logger, violations: usize) {
    slog::warn!(
        log,
        "Sending stats is disabled, but there is no valid Bencher Plus license key! This is violation #{violations} of the Bencher License: https://bencher.dev/legal/license"
    );
}

pub(super) fn configure_standalone_connection(
    conn: &mut DbConnection,
    busy_timeout: u32,
) -> diesel::QueryResult<()> {
    conn.batch_execute(&format!("PRAGMA busy_timeout = {busy_timeout}"))?;
    conn.batch_execute("PRAGMA synchronous = NORMAL")?;
    conn.batch_execute("PRAGMA extended_result_codes = ON")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{LICENSE_GRACE_PERIOD, LicenseGrace, OptOut};

    // Fails if a server without a valid license skips its stats for more or fewer days than the
    // grace period, a valid license does not stop the count, or the grace does not start again
    // from the beginning once a valid license appears and then lapses.
    #[test]
    fn the_stats_grace_starts_again_when_a_license_appears_and_lapses() {
        assert_eq!(LICENSE_GRACE_PERIOD, 7);
        let mut grace = LicenseGrace::default();
        for (day, remaining) in (0..LICENSE_GRACE_PERIOD).rev().enumerate() {
            assert_eq!(
                grace.check(false),
                OptOut::Grace {
                    violations: day + 1,
                    remaining,
                }
            );
        }
        assert_eq!(grace.check(false), OptOut::Send { violations: 8 });
        assert_eq!(grace.check(false), OptOut::Send { violations: 9 });

        assert_eq!(grace.check(true), OptOut::Licensed);
        assert_eq!(
            grace.check(false),
            OptOut::Grace {
                violations: 1,
                remaining: LICENSE_GRACE_PERIOD - 1,
            }
        );
    }
}
