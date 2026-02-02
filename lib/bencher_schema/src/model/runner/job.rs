use bencher_json::{DateTime, JobStatus, JobUuid, JsonJob, RunnerUuid};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    model::{
        project::report::ReportId,
        runner::{QueryRunner, RunnerId},
    },
    resource_not_found_err,
    schema::{self, job as job_table},
};

crate::macros::typed_id::typed_id!(JobId);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = job_table)]
#[diesel(belongs_to(QueryRunner, foreign_key = runner_id))]
pub struct QueryJob {
    pub id: JobId,
    pub uuid: JobUuid,
    pub report_id: ReportId,
    pub status: JobStatus,
    pub spec: String,
    pub timeout: i32,
    pub priority: i32,
    pub runner_id: Option<RunnerId>,
    pub claimed: Option<DateTime>,
    pub started: Option<DateTime>,
    pub completed: Option<DateTime>,
    pub last_heartbeat: Option<DateTime>,
    pub last_billed_minute: Option<i32>,
    pub exit_code: Option<i32>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryJob {
    pub fn get(conn: &mut DbConnection, id: JobId) -> Result<Self, HttpError> {
        schema::job::table
            .filter(schema::job::id.eq(id))
            .first(conn)
            .map_err(resource_not_found_err!(Job, id))
    }

    pub fn from_uuid(conn: &mut DbConnection, uuid: JobUuid) -> Result<Self, HttpError> {
        schema::job::table
            .filter(schema::job::uuid.eq(uuid))
            .first(conn)
            .map_err(resource_not_found_err!(Job, uuid))
    }

    pub fn into_json(self, runner_uuid: RunnerUuid) -> JsonJob {
        JsonJob {
            uuid: self.uuid,
            status: self.status,
            runner: Some(runner_uuid),
            claimed: self.claimed,
            started: self.started,
            completed: self.completed,
            exit_code: self.exit_code,
            created: self.created,
            modified: self.modified,
        }
    }

    pub fn into_json_for_project(self, conn: &mut DbConnection) -> Result<JsonJob, HttpError> {
        let runner_uuid = if let Some(runner_id) = self.runner_id {
            QueryRunner::get(conn, runner_id).ok().map(|r| r.uuid)
        } else {
            None
        };

        Ok(JsonJob {
            uuid: self.uuid,
            status: self.status,
            runner: runner_uuid,
            claimed: self.claimed,
            started: self.started,
            completed: self.completed,
            exit_code: self.exit_code,
            created: self.created,
            modified: self.modified,
        })
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = job_table)]
pub struct InsertJob {
    pub uuid: JobUuid,
    pub report_id: ReportId,
    pub status: JobStatus,
    pub spec: String,
    pub timeout: i32,
    pub priority: i32,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertJob {
    pub fn new(report_id: ReportId, spec: String, timeout: i32, priority: i32) -> Self {
        let now = DateTime::now();
        Self {
            uuid: JobUuid::new(),
            report_id,
            status: JobStatus::default(),
            spec,
            timeout,
            priority,
            created: now,
            modified: now,
        }
    }
}

#[derive(Debug, Default, diesel::AsChangeset)]
#[diesel(table_name = job_table)]
pub struct UpdateJob {
    pub status: Option<JobStatus>,
    pub runner_id: Option<Option<RunnerId>>,
    pub claimed: Option<Option<DateTime>>,
    pub started: Option<Option<DateTime>>,
    pub completed: Option<Option<DateTime>>,
    pub last_heartbeat: Option<Option<DateTime>>,
    pub last_billed_minute: Option<Option<i32>>,
    pub exit_code: Option<Option<i32>>,
    pub modified: Option<DateTime>,
}
