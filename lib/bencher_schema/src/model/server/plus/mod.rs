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
    model::{
        organization::license::{ServerLicense, any_valid_license, server_license},
        user::QueryUser,
    },
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

    /// The daily stats loop of this server, ready to spawn.
    #[expect(clippy::too_many_arguments, reason = "the settings of the stats loop")]
    pub fn daily_stats(
        self,
        log: Logger,
        db_path: PathBuf,
        busy_timeout: u32,
        stats: StatsSettings,
        messenger: Option<Messenger>,
        licensor: Licensor,
        is_bencher_cloud: bool,
        disaster_recovery: bool,
    ) -> Result<DailyStats, HttpError> {
        let server_stats_url = self
            .server_stats_url()
            .map_err(resource_not_found_err!(Server, &db_path))?;
        slog::info!(log, "Server stats endpoint: {server_stats_url}");
        let StatsSettings { offset, enabled } = stats;
        Ok(DailyStats {
            server: self,
            log,
            db_path,
            busy_timeout,
            server_stats_url,
            offset,
            enabled,
            messenger,
            licensor,
            is_bencher_cloud,
            disaster_recovery,
            grace: LicenseGrace::default(),
        })
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

/// The daily stats loop of a server: once a day, at its offset, it checks the licenses, and sends
/// the server's stats unless a Self-Hosted server with a valid license has turned them off.
pub struct DailyStats {
    server: QueryServer,
    log: Logger,
    db_path: PathBuf,
    busy_timeout: u32,
    server_stats_url: Url,
    offset: chrono::NaiveTime,
    enabled: bool,
    messenger: Option<Messenger>,
    licensor: Licensor,
    is_bencher_cloud: bool,
    disaster_recovery: bool,
    grace: LicenseGrace,
}

impl DailyStats {
    /// Run the loop for as long as the server runs.
    pub fn spawn(mut self) {
        tokio::spawn(async move {
            if !self.is_bencher_cloud {
                let client = reqwest::Client::new();
                if let Err(e) = client
                    .get(self.server_stats_url.clone())
                    .query(&BooleanParam::True(SelfHostedStartup))
                    .send()
                    .await
                {
                    slog::warn!(self.log, "Failed to register startup: {e}");
                }
            }

            loop {
                let now = Utc::now().naive_utc().time();
                let sleep_time = match now.cmp(&self.offset) {
                    cmp::Ordering::Less => self.offset - now,
                    cmp::Ordering::Equal => Duration::days(1),
                    cmp::Ordering::Greater => Duration::days(1) - (now - self.offset),
                }
                .to_std()
                .unwrap_or(std::time::Duration::from_hours(24));
                tokio::time::sleep(sleep_time).await;

                self.run(DateTime::now()).await;
            }
        });
    }

    /// One day's work of the loop, at `now`.
    async fn run(&mut self, now: DateTime) {
        let Some(mut conn) = self.connect() else {
            return;
        };
        self.warn_replication(&mut conn, now);
        if self.skips_stats(&mut conn, now) {
            return;
        }
        self.send_stats(conn).await;
    }

    /// A database connection of the loop's own, or none once it logs why.
    fn connect(&self) -> Option<DbConnection> {
        let Ok(mut conn) = DbConnection::establish(self.db_path.to_string_lossy().as_ref()) else {
            slog::error!(self.log, "Failed to establish database connection");
            return None;
        };
        if let Err(e) = configure_standalone_connection(&mut conn, self.busy_timeout) {
            slog::error!(
                self.log,
                "Failed to configure database connection PRAGMAs: {e}"
            );
            return None;
        }
        Some(conn)
    }

    /// Log when the next start will refuse. Only a Self-Hosted server that replicates needs a
    /// license to start again.
    fn warn_replication(&self, conn: &mut DbConnection, now: DateTime) {
        if !self.disaster_recovery || self.is_bencher_cloud {
            return;
        }
        match server_license(conn, &self.licensor, now) {
            Ok(license) => {
                if let Some(warning) = replication_warning(license) {
                    slog::error!(self.log, "{warning}");
                }
            },
            Err(e) => slog::error!(
                self.log,
                "Failed to check the licenses for disaster recovery: {e}"
            ),
        }
    }

    /// Whether today's stats stay unsent: a Self-Hosted server that has turned them off skips
    /// them with a valid license, and without one while its grace lasts.
    fn skips_stats(&mut self, conn: &mut DbConnection, now: DateTime) -> bool {
        if self.enabled {
            slog::info!(self.log, "Sending stats at {}", Utc::now());
            return false;
        }
        if self.is_bencher_cloud {
            slog::info!(
                self.log,
                "Sending stats is disabled, but running on Bencher Cloud"
            );
            return false;
        }
        let licensed = match any_valid_license(conn, &self.licensor, now) {
            Ok(licensed) => licensed,
            Err(e) => {
                slog::error!(self.log, "Failed to check stats: {e}");
                return true;
            },
        };
        match self.grace.check(licensed) {
            OptOut::Licensed => {
                slog::debug!(self.log, "Sending stats is disabled");
                true
            },
            OptOut::Grace {
                violations,
                remaining,
            } => {
                warn_violation(&self.log, violations);
                slog::warn!(
                    self.log,
                    "You have {remaining} days remaining in your Bencher License grace period. Please purchase a license key: https://bencher.dev/pricing"
                );
                true
            },
            OptOut::Send { violations } => {
                warn_violation(&self.log, violations);
                slog::warn!(
                    self.log,
                    "Sending stats at {}. Please purchase a license key: https://bencher.dev/pricing",
                    Utc::now()
                );
                false
            },
        }
    }

    /// Send today's stats: by email on Bencher Cloud, and to Bencher Cloud from Self-Hosted.
    async fn send_stats(&self, conn: DbConnection) {
        let log = &self.log;
        if !self.is_bencher_cloud {
            let client = reqwest::Client::new();
            if let Err(e) = client
                .get(self.server_stats_url.clone())
                .query(&BooleanParam::True(SelfHostedStats))
                .send()
                .await
            {
                slog::warn!(log, "Failed to register stats: {e}");
            }
        }

        let Some(json_stats_str) = self
            .server
            .get_stats_str(log, conn, self.is_bencher_cloud)
            .await
        else {
            slog::error!(log, "Failed to get stats string");
            return;
        };

        if let Some(messenger) = self.messenger.as_ref() {
            slog::info!(log, "Bencher Cloud Stats: {json_stats_str:?}");

            let Ok(mut conn) = DbConnection::establish(self.db_path.to_string_lossy().as_ref())
            else {
                slog::error!(
                    log,
                    "Failed to establish database connection for sending stats"
                );
                return;
            };

            if let Err(e) = configure_standalone_connection(&mut conn, self.busy_timeout) {
                slog::error!(
                    log,
                    "Failed to configure database connection PRAGMAs for sending stats: {e}"
                );
                return;
            }

            if let Err(e) =
                QueryServer::send_stats_to_backend(log, &mut conn, messenger, &json_stats_str, None)
            {
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

/// What the daily stats loop logs on a Self-Hosted server that replicates for disaster recovery,
/// which needs a valid license to start again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplicationWarning {
    /// Every valid license is in its grace, so a start after `expiration` refuses.
    Grace { expiration: DateTime },
    /// No license is valid, so the next start refuses.
    Lapsed,
}

impl std::fmt::Display for ReplicationWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Grace { expiration } => write!(
                f,
                "Disaster recovery (`plus.disaster_recovery`) is replicating, but every Bencher Plus Enterprise license on this server is past its term. After {expiration}, this server will refuse to start without a valid license: https://bencher.dev/pricing"
            ),
            Self::Lapsed => write!(
                f,
                "Disaster recovery (`plus.disaster_recovery`) is replicating, but no organization on this server holds a valid Bencher Plus Enterprise license. The next time it starts, this server will refuse to start without one: https://bencher.dev/pricing"
            ),
        }
    }
}

/// The warning for a Self-Hosted server that replicates, from what its licenses say.
fn replication_warning(license: ServerLicense) -> Option<ReplicationWarning> {
    match license {
        ServerLicense::InTerm => None,
        ServerLicense::InGrace { expiration } => Some(ReplicationWarning::Grace { expiration }),
        ServerLicense::Unlicensed => Some(ReplicationWarning::Lapsed),
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
    use std::sync::{Arc, Mutex};

    use bencher_json::{DateTime, Entitlements, Jwt, OrganizationUuid, Secret};
    use bencher_license::Licensor;
    use chrono::{Duration, NaiveTime};
    use diesel::{Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
    use pretty_assertions::assert_eq;
    use slog::Drain as _;

    use super::{
        LICENSE_GRACE_PERIOD, LicenseGrace, OptOut, QueryServer, ReplicationWarning,
        replication_warning,
    };
    use crate::{
        context::{DbConnection, Messenger, StatsSettings},
        model::organization::license::ServerLicense,
        run_migrations, schema,
        test_util::create_base_entities,
    };

    // Debug builds trust this key's public half for Self-Hosted licenses.
    const TEST_LICENSE_KEY: &str =
        include_str!("../../../../../../plus/bencher_license/src/test/private.pem");

    /// Keeps the message of every log line.
    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<String>>>);

    impl slog::Drain for Capture {
        type Ok = ();
        type Err = slog::Never;

        fn log(
            &self,
            record: &slog::Record<'_>,
            _values: &slog::OwnedKVList,
        ) -> Result<(), slog::Never> {
            if let Ok(mut lines) = self.0.lock() {
                lines.push(record.msg().to_string());
            }
            Ok(())
        }
    }

    /// A server's license, by where `DateTime::TEST` falls in it.
    #[derive(Clone, Copy)]
    enum Held {
        None,
        InTerm,
        InGrace,
    }

    /// The lines that one day of the stats loop logs, with stats turned off, on a server whose
    /// one organization holds `held`.
    async fn one_day(held: Held, is_bencher_cloud: bool, disaster_recovery: bool) -> Vec<String> {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("bencher.db");
        let mut conn = DbConnection::establish(db_path.to_str().unwrap()).unwrap();
        run_migrations(&mut conn).unwrap();
        let organization_id = create_base_entities(&mut conn).organization_id;
        let organization: OrganizationUuid = schema::organization::table
            .find(organization_id)
            .select(schema::organization::uuid)
            .first(&mut conn)
            .unwrap();
        let term_end = match held {
            Held::None => None,
            Held::InTerm => Some(DateTime::TEST + Duration::days(30)),
            Held::InGrace => Some(DateTime::TEST - Duration::days(1)),
        };
        if let Some(term_end) = term_end {
            let key: Secret = TEST_LICENSE_KEY.parse().unwrap();
            let license: Jwt = Licensor::bencher_cloud(&key)
                .unwrap()
                .new_license(
                    organization,
                    Entitlements::MIN,
                    term_end - Duration::days(365),
                    term_end,
                )
                .unwrap();
            diesel::update(schema::organization::table.find(organization_id))
                .set(schema::organization::license.eq(license))
                .execute(&mut conn)
                .unwrap();
        }
        let server = QueryServer::get_or_create(&mut conn).unwrap();
        drop(conn);

        let capture = Capture::default();
        let log = slog::Logger::root(capture.clone().fuse(), slog::o!());
        let mut daily = server
            .daily_stats(
                log,
                db_path,
                5_000,
                StatsSettings {
                    offset: NaiveTime::MIN,
                    enabled: false,
                },
                is_bencher_cloud.then(Messenger::default),
                Licensor::self_hosted().unwrap(),
                is_bencher_cloud,
                disaster_recovery,
            )
            .unwrap();
        daily.run(DateTime::TEST).await;
        capture.0.lock().unwrap().clone()
    }

    fn replication(lines: &[String]) -> Vec<String> {
        lines
            .iter()
            .filter(|line| line.contains("`plus.disaster_recovery`"))
            .cloned()
            .collect()
    }

    // Fails if a day of the stats loop on a Self-Hosted server that replicates does not warn that
    // the next start refuses once no license is valid, or when a start will refuse once every
    // license is in its grace; or if it warns while a license is in its term, on a server that does
    // not replicate, or on Bencher Cloud, which never needs a license to replicate.
    #[tokio::test]
    async fn a_day_of_stats_warns_a_replicating_server_without_a_license() {
        let lapsed = replication(&one_day(Held::None, false, true).await);
        assert_eq!(lapsed.len(), 1, "{lapsed:?}");
        assert!(
            lapsed
                .iter()
                .all(|line| line.contains("next time it starts")),
            "{lapsed:?}"
        );
        let grace = replication(&one_day(Held::InGrace, false, true).await);
        assert_eq!(grace.len(), 1, "{grace:?}");
        assert!(
            grace.iter().all(|line| line.contains("past its term")),
            "{grace:?}"
        );

        assert_eq!(
            replication(&one_day(Held::InTerm, false, true).await),
            Vec::<String>::new()
        );
        assert_eq!(
            replication(&one_day(Held::None, false, false).await),
            Vec::<String>::new()
        );
        assert_eq!(
            replication(&one_day(Held::None, true, true).await),
            Vec::<String>::new()
        );
    }

    // Fails if a server with a license in its term warns about disaster recovery, or one whose
    // licenses are all in their grace, or that has none, does not warn that a start after the
    // grace, or the next start, refuses, with the license and the configuration key named.
    #[test]
    fn replication_warns_from_the_first_day_of_grace() {
        let expiration = DateTime::TEST;
        assert_eq!(replication_warning(ServerLicense::InTerm), None);
        assert_eq!(
            replication_warning(ServerLicense::InGrace { expiration }),
            Some(ReplicationWarning::Grace { expiration })
        );
        assert_eq!(
            replication_warning(ServerLicense::Unlicensed),
            Some(ReplicationWarning::Lapsed)
        );

        let grace = ReplicationWarning::Grace { expiration }.to_string();
        assert!(grace.contains(&expiration.to_string()), "{grace}");
        let lapsed = ReplicationWarning::Lapsed.to_string();
        assert!(lapsed.contains("next time it starts"), "{lapsed}");
        for warning in [grace, lapsed] {
            assert!(warning.contains("`plus.disaster_recovery`"), "{warning}");
            assert!(
                warning.contains("Bencher Plus Enterprise license"),
                "{warning}"
            );
        }
    }

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
