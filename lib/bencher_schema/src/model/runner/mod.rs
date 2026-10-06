use std::string::ToString as _;

use bencher_json::{
    DateTime, JsonRunner, JsonUpdateRunner, ResourceName, RunnerKeyHash, RunnerSlug, SpecUuid,
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    result::QueryResult,
};
use dropshot::HttpError;
use slog::Logger;

pub use bencher_json::{JobStatus, JobUuid, RunnerUuid};

use crate::{
    context::DbConnection,
    macros::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
    },
    model::spec::QuerySpec,
    schema::{self, runner as runner_table},
};

/// How often, at most, a runner's `last_heartbeat` is written.
const LAST_HEARTBEAT_INTERVAL_SECS: i64 = 300;

pub mod job;
pub mod job_callback;
pub mod runner_spec;
mod source_ip;
pub mod status;

pub use job::{
    InsertJob, JobId, JobTimeout, PendingInsertJob, QueryJob, UpdateJob, in_flight_jobs,
    mark_orphaned_claimed_jobs_unknown, reprocess_completed_jobs,
};
pub use job_callback::{
    CallbackOutcome, HttpStatus, InsertJobCallback, QueryJobCallback, QueryJobCallbackView,
};
pub use runner_spec::{InsertRunnerSpec, QueryRunnerSpec, RunnerSpecId};
pub use source_ip::SourceIp;
pub use status::{PAUSE_NOTICE_HOURS, QueryRunnerStatus, RunnerNotice, RunnerReport, StatusChange};

crate::macros::typed_id::typed_id!(RunnerId);

#[derive(Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Selectable)]
#[diesel(table_name = runner_table)]
pub struct QueryRunner {
    pub id: RunnerId,
    pub uuid: RunnerUuid,
    pub name: ResourceName,
    pub slug: RunnerSlug,
    pub key_hash: RunnerKeyHash,
    pub last_heartbeat: Option<DateTime>,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl QueryRunner {
    fn_get!(runner, RunnerId);
    fn_get_id!(runner, RunnerId, RunnerUuid);
    fn_get_uuid!(runner, RunnerId, RunnerUuid);
    fn_from_uuid!(runner, RunnerUuid, Runner);
    fn_eq_resource_id!(runner, RunnerResourceId);
    fn_from_resource_id!(runner, Runner, RunnerResourceId);

    pub fn is_archived(&self) -> bool {
        self.archived.is_some()
    }

    pub fn into_json(self, log: &Logger, conn: &mut DbConnection) -> Result<JsonRunner, HttpError> {
        let spec_ids = QueryRunnerSpec::spec_ids_for_runner(conn, self.id)?;
        let specs: Vec<SpecUuid> = spec_ids
            .into_iter()
            .map(|spec_id| QuerySpec::get(conn, spec_id).map(|s| s.uuid))
            .collect::<Result<_, _>>()?;
        // An unreadable status shows as none, so the runner and the runner list still load.
        let status = QueryRunnerStatus::get(conn, self.id)
            .inspect_err(|e| {
                slog::warn!(log, "Failed to read runner status"; "runner" => %self.uuid, "error" => %e);
            })
            .ok()
            .flatten()
            .map(QueryRunnerStatus::into_json);
        Ok(JsonRunner {
            uuid: self.uuid,
            name: self.name,
            slug: self.slug,
            specs,
            archived: self.archived,
            last_heartbeat: self.last_heartbeat,
            status,
            created: self.created,
            modified: self.modified,
        })
    }

    /// Write `now` as the runner's `last_heartbeat`, unless it was written in the last five minutes.
    pub fn record_heartbeat(
        conn: &mut DbConnection,
        runner_id: RunnerId,
        now: DateTime,
    ) -> QueryResult<usize> {
        let stale = now.timestamp().saturating_sub(LAST_HEARTBEAT_INTERVAL_SECS);
        diesel::update(
            schema::runner::table
                .filter(schema::runner::id.eq(runner_id))
                .filter(
                    schema::runner::last_heartbeat
                        .is_null()
                        .or(schema::runner::last_heartbeat.le(stale)),
                ),
        )
        .set(schema::runner::last_heartbeat.eq(now))
        .execute(conn)
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = runner_table)]
pub struct InsertRunner {
    pub uuid: RunnerUuid,
    pub name: ResourceName,
    pub slug: RunnerSlug,
    pub key_hash: RunnerKeyHash,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertRunner {
    pub fn new(
        name: ResourceName,
        slug: RunnerSlug,
        key_hash: RunnerKeyHash,
        now: DateTime,
    ) -> Self {
        Self {
            uuid: RunnerUuid::new(),
            name,
            slug,
            key_hash,
            created: now,
            modified: now,
        }
    }
}

#[derive(Debug, Default, diesel::AsChangeset)]
#[diesel(table_name = runner_table)]
pub struct UpdateRunner {
    pub name: Option<ResourceName>,
    pub slug: Option<RunnerSlug>,
    pub key_hash: Option<RunnerKeyHash>,
    pub last_heartbeat: Option<Option<DateTime>>,
    pub modified: Option<DateTime>,
    pub archived: Option<Option<DateTime>>,
}

impl UpdateRunner {
    pub fn from_json(update: JsonUpdateRunner, now: DateTime) -> Self {
        let JsonUpdateRunner {
            name,
            slug,
            archived,
        } = update;
        let archived = archived.map(|archived| archived.then_some(now));
        Self {
            name,
            slug,
            archived,
            modified: Some(now),
            ..Default::default()
        }
    }
}
