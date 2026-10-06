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

use super::RunnerId;
use crate::{
    context::DbConnection,
    schema::{self, runner_status as runner_status_table},
};

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

impl QueryRunnerStatus {
    pub fn get(conn: &mut DbConnection, runner_id: RunnerId) -> QueryResult<Option<Self>> {
        schema::runner_status::table
            .filter(schema::runner_status::runner_id.eq(runner_id))
            .select(Self::as_select())
            .first(conn)
            .optional()
    }

    /// Store the report if it changes the runner's availability, pause reason kinds, health state, or findings.
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
