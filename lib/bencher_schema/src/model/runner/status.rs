use std::{collections::HashSet, mem};

use bencher_json::{
    DateTime,
    runner::{
        HealthState, JsonHealthFinding, JsonPaused, JsonReady, JsonRunnerHealth, JsonRunnerStatus,
        MAX_PAUSE_REASONS, PauseReason, RunnerAvailability,
    },
};
use diesel::{
    ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _,
    SelectableHelper as _,
    deserialize::{self, FromSql},
    result::QueryResult,
    serialize::{self, IsNull, Output, ToSql},
    sql_types::Text,
    sqlite::{Sqlite, SqliteValue},
};
use dropshot::HttpError;
use slog::Logger;

use super::RunnerId;
use crate::{
    context::{Body, DbConnection, Message, Messenger, RunnerStatusBody},
    model::user::QueryUser,
    schema::{self, runner_status as runner_status_table},
};

/// How long a runner stays paused before server admins hear about it.
pub const PAUSE_NOTICE_HOURS: i64 = 12;
const PAUSE_NOTICE_SECS: i64 = PAUSE_NOTICE_HOURS * 60 * 60;

/// A runner's status, one row per runner, written only when it changes.
#[derive(Debug, Clone, diesel::Queryable, diesel::Selectable, diesel::Insertable)]
#[diesel(table_name = runner_status_table)]
pub struct QueryRunnerStatus {
    pub runner_id: RunnerId,
    pub availability: RunnerAvailability,
    pub reasons: StoredReasons,
    pub since: Option<DateTime>,
    pub health: Option<StoredHealth>,
    pub changed: DateTime,
}

/// What a runner says about itself with each `Ready` or `Paused`.
#[derive(Debug, Clone)]
pub struct RunnerReport {
    pub availability: RunnerAvailability,
    pub reasons: Vec<PauseReason>,
    pub since: Option<DateTime>,
    pub health: Option<JsonRunnerHealth>,
}

/// A stored change: the status it replaced, if any, and the status it stored.
#[derive(Debug, Clone)]
pub struct StatusChange {
    pub old: Option<QueryRunnerStatus>,
    pub new: QueryRunnerStatus,
}

/// What server admins hear about a runner.
#[derive(Debug, Clone)]
pub enum RunnerNotice {
    /// Its disk health changed state, or gained a finding while not `ok`.
    Health {
        was: Option<HealthState>,
        health: JsonRunnerHealth,
        new: Vec<JsonHealthFinding>,
    },
    /// It has been paused for longer than `PAUSE_NOTICE_HOURS`.
    LongPause {
        since: DateTime,
        reasons: Vec<PauseReason>,
    },
    /// It takes jobs again after a long pause.
    PauseEnded { since: DateTime },
}

impl QueryRunnerStatus {
    pub fn get(conn: &mut DbConnection, runner_id: RunnerId) -> QueryResult<Option<Self>> {
        schema::runner_status::table
            .filter(schema::runner_status::runner_id.eq(runner_id))
            .select(Self::as_select())
            .first(conn)
            .optional()
    }

    /// Store the report if it changes the runner's availability, pause reason kinds, health state or findings,
    /// or long pause.
    pub fn record(
        conn: &mut DbConnection,
        runner_id: RunnerId,
        report: RunnerReport,
        now: DateTime,
    ) -> QueryResult<Option<StatusChange>> {
        let old = match Self::get(conn, runner_id) {
            Ok(old) => old,
            // A row this build cannot read is replaced, so the runner's status recovers.
            Err(diesel::result::Error::DeserializationError(_)) => {
                diesel::delete(
                    schema::runner_status::table
                        .filter(schema::runner_status::runner_id.eq(runner_id)),
                )
                .execute(conn)?;
                None
            },
            Err(e) => return Err(e),
        };
        let new = report.into_status(runner_id, old.as_ref(), now);
        let written = if let Some(old) = &old {
            if !new.differs_from(old) {
                return Ok(None);
            }
            diesel::update(
                schema::runner_status::table
                    .filter(schema::runner_status::runner_id.eq(runner_id))
                    .filter(schema::runner_status::changed.eq(old.changed)),
            )
            .set((
                schema::runner_status::availability.eq(new.availability),
                schema::runner_status::reasons.eq(&new.reasons),
                schema::runner_status::since.eq(new.since),
                schema::runner_status::health.eq(&new.health),
                schema::runner_status::changed.eq(new.changed),
            ))
            .execute(conn)?
        } else {
            diesel::insert_into(schema::runner_status::table)
                .values(&new)
                .on_conflict_do_nothing()
                .execute(conn)?
        };
        Ok((written > 0).then_some(StatusChange { old, new }))
    }

    pub fn into_json(self) -> JsonRunnerStatus {
        let Self {
            runner_id: _,
            availability,
            reasons,
            since,
            health,
            changed,
        } = self;
        JsonRunnerStatus {
            availability,
            reasons: reasons.0,
            since,
            health: health.map(|health| health.0),
            changed,
        }
    }

    fn differs_from(&self, old: &Self) -> bool {
        self.availability != old.availability
            || self.reasons.kinds() != old.reasons.kinds()
            || self.health.as_ref().map(StoredHealth::coarse)
                != old.health.as_ref().map(StoredHealth::coarse)
            || self.long_pause() != old.long_pause()
    }

    // Measured to `changed`, so a stored row stays long once it is written long.
    fn long_pause(&self) -> bool {
        self.availability == RunnerAvailability::Paused
            && self.since.is_some_and(|since| {
                self.changed.timestamp().saturating_sub(since.timestamp()) > PAUSE_NOTICE_SECS
            })
    }
}

impl RunnerReport {
    pub fn ready(ready: &JsonReady) -> Self {
        Self {
            availability: RunnerAvailability::Ready,
            reasons: Vec::new(),
            since: None,
            health: ready.health.clone(),
        }
    }

    pub fn paused(paused: &JsonPaused) -> Self {
        Self {
            availability: RunnerAvailability::Paused,
            reasons: paused.reasons.clone(),
            since: Some(paused.since),
            health: paused.ready.health.clone(),
        }
    }

    fn into_status(
        self,
        runner_id: RunnerId,
        old: Option<&QueryRunnerStatus>,
        now: DateTime,
    ) -> QueryRunnerStatus {
        let Self {
            availability,
            mut reasons,
            since,
            health,
        } = self;
        reasons.truncate(MAX_PAUSE_REASONS);
        for reason in &mut reasons {
            reason.cap();
        }
        let health = if let Some(mut health) = health {
            health.cap();
            Some(StoredHealth(health))
        } else {
            old.and_then(|old| old.health.clone())
        };
        // A runner that restarts mid-pause reports a new start; the server's pause has not ended.
        let since = if let Some(old) = old
            && old.availability == RunnerAvailability::Paused
            && availability == RunnerAvailability::Paused
        {
            old.since
        } else {
            since
        };
        QueryRunnerStatus {
            runner_id,
            availability,
            reasons: StoredReasons(reasons),
            since,
            health,
            changed: now,
        }
    }
}

impl StatusChange {
    /// Mail every server admin about this change, if it is news.
    pub fn notify(
        &self,
        log: &Logger,
        conn: &mut DbConnection,
        messenger: &Messenger,
        runner: &str,
    ) -> Result<(), HttpError> {
        let notices = self.notices();
        if notices.is_empty() {
            return Ok(());
        }
        let admins = QueryUser::get_admins(conn)?;
        for notice in notices {
            for admin in &admins {
                let body = RunnerStatusBody {
                    admin: admin.name.clone().into(),
                    runner: runner.to_owned(),
                    notice: notice.clone(),
                };
                let message = Message {
                    to_name: Some(admin.name.clone().into()),
                    to_email: admin.email.clone().into(),
                    subject: Some(body.subject()),
                    body: Some(Body::RunnerStatus(body)),
                };
                messenger.send(log, message);
            }
        }
        Ok(())
    }

    /// Mail on a health state change, a new finding while not `ok`, a pause that grew long, and a long pause's end.
    pub fn notices(&self) -> Vec<RunnerNotice> {
        let mut notices = Vec::new();
        if let Some(notice) = self.health_notice() {
            notices.push(notice);
        }
        let old_long = self.old.as_ref().is_some_and(QueryRunnerStatus::long_pause);
        if !old_long
            && self.new.long_pause()
            && let Some(since) = self.new.since
        {
            notices.push(RunnerNotice::LongPause {
                since,
                reasons: self.new.reasons.0.clone(),
            });
        }
        if old_long
            && self.new.availability == RunnerAvailability::Ready
            && let Some(since) = self.old.as_ref().and_then(|old| old.since)
        {
            notices.push(RunnerNotice::PauseEnded { since });
        }
        notices
    }

    fn health_notice(&self) -> Option<RunnerNotice> {
        let health = &self.new.health.as_ref()?.0;
        if !is_known(&health.state) {
            return None;
        }
        let old = self
            .old
            .as_ref()
            .and_then(|old| old.health.as_ref())
            .map(|old| &old.0);
        let was = old
            .map(|old| &old.state)
            .filter(|state| is_known(state))
            .cloned();
        let new: Vec<JsonHealthFinding> = health
            .findings
            .iter()
            .filter(|finding| {
                !old.is_some_and(|old| {
                    old.findings
                        .iter()
                        .any(|seen| seen.device == finding.device && seen.kind == finding.kind)
                })
            })
            .cloned()
            .collect();
        let state_changed = was.as_ref() != Some(&health.state)
            && (was.is_some() || health.state != HealthState::Ok);
        let new_trouble = health.state != HealthState::Ok && !new.is_empty();
        (state_changed || new_trouble).then(|| RunnerNotice::Health {
            was,
            health: health.clone(),
            new,
        })
    }
}

fn is_known(state: &HealthState) -> bool {
    matches!(
        state,
        HealthState::Ok | HealthState::Warning | HealthState::Failing
    )
}

/// Pause reasons, stored as JSON.
#[derive(Debug, Clone, diesel::AsExpression, diesel::FromSqlRow)]
#[diesel(sql_type = Text)]
pub struct StoredReasons(pub Vec<PauseReason>);

impl StoredReasons {
    fn kinds(&self) -> HashSet<mem::Discriminant<PauseReason>> {
        self.0.iter().map(mem::discriminant).collect()
    }
}

/// A health report, stored as JSON.
#[derive(Debug, Clone, diesel::AsExpression, diesel::FromSqlRow)]
#[diesel(sql_type = Text)]
pub struct StoredHealth(pub JsonRunnerHealth);

impl StoredHealth {
    fn coarse(&self) -> (&HealthState, &[JsonHealthFinding]) {
        (&self.0.state, &self.0.findings)
    }
}

impl ToSql<Text, Sqlite> for StoredReasons {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
        out.set_value(serde_json::to_string(&self.0)?);
        Ok(IsNull::No)
    }
}

impl FromSql<Text, Sqlite> for StoredReasons {
    fn from_sql(bytes: SqliteValue<'_, '_, '_>) -> deserialize::Result<Self> {
        let json = <String as FromSql<Text, Sqlite>>::from_sql(bytes)?;
        Ok(Self(serde_json::from_str(&json)?))
    }
}

impl ToSql<Text, Sqlite> for StoredHealth {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
        out.set_value(serde_json::to_string(&self.0)?);
        Ok(IsNull::No)
    }
}

impl FromSql<Text, Sqlite> for StoredHealth {
    fn from_sql(bytes: SqliteValue<'_, '_, '_>) -> deserialize::Result<Self> {
        let json = <String as FromSql<Text, Sqlite>>::from_sql(bytes)?;
        Ok(Self(serde_json::from_str(&json)?))
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, runner::RunnerAvailability};
    use diesel::{RunQueryDsl as _, connection::SimpleConnection as _};
    use diesel_migrations::MigrationHarness as _;

    use super::{QueryRunnerStatus, RunnerReport};
    use crate::{model::runner::RunnerId, test_util::setup_test_db};

    // Kills a down migration that leaves the table behind, which the up migration then trips over.
    #[test]
    fn migration_down_and_up_drops_and_restores_the_table() {
        let mut conn = setup_test_db();
        diesel::sql_query(
            "INSERT INTO runner (id, uuid, name, slug, key_hash, created, modified)
                VALUES (1, '00000000-0000-0000-0000-000000000070', 'Runner', 'runner', 'hash', 0, 0)",
        )
        .execute(&mut conn)
        .unwrap();
        let runner_id = RunnerId::from_raw(1);
        let report = RunnerReport {
            availability: RunnerAvailability::Ready,
            reasons: Vec::new(),
            since: None,
            health: None,
        };
        QueryRunnerStatus::record(&mut conn, runner_id, report, DateTime::TEST).unwrap();

        // Foreign keys are off as in `run_migrations`, so a later migration that rebuilds a table reverts.
        conn.batch_execute("PRAGMA foreign_keys = OFF").unwrap();
        loop {
            let version = conn.revert_last_migration(crate::MIGRATIONS).unwrap();
            if version.to_string() == "20261006120000" {
                break;
            }
        }
        assert!(
            QueryRunnerStatus::get(&mut conn, runner_id).is_err(),
            "the down migration drops the table"
        );

        conn.run_pending_migrations(crate::MIGRATIONS).unwrap();
        conn.batch_execute("PRAGMA foreign_keys = ON").unwrap();
        assert!(
            QueryRunnerStatus::get(&mut conn, runner_id)
                .unwrap()
                .is_none(),
            "the up migration restores an empty table"
        );
    }
}
