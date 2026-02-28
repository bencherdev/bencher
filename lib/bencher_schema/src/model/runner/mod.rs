use std::string::ToString as _;

use bencher_json::{DateTime, JsonRunner, JsonUpdateRunner, ResourceName, RunnerSlug, SpecUuid};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

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

pub mod job;
pub mod runner_spec;
mod source_ip;
mod token_hash;

#[cfg(feature = "plus")]
pub use job::reprocess_completed_jobs;
pub use job::{
    InsertJob, JobId, PendingInsertJob, QueryJob, UpdateJob, recover_orphaned_claimed_jobs,
    spawn_heartbeat_timeout,
};
pub use runner_spec::{InsertRunnerSpec, QueryRunnerSpec, RunnerSpecId};
pub use source_ip::SourceIp;
pub use token_hash::TokenHash;

crate::macros::typed_id::typed_id!(RunnerId);

#[derive(Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Selectable)]
#[diesel(table_name = runner_table)]
pub struct QueryRunner {
    pub id: RunnerId,
    pub uuid: RunnerUuid,
    pub name: ResourceName,
    pub slug: RunnerSlug,
    pub token_hash: TokenHash,
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

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonRunner, HttpError> {
        let spec_ids = QueryRunnerSpec::spec_ids_for_runner(conn, self.id)?;
        let specs: Vec<SpecUuid> = spec_ids
            .into_iter()
            .map(|spec_id| QuerySpec::get(conn, spec_id).map(|s| s.uuid))
            .collect::<Result<_, _>>()?;
        Ok(JsonRunner {
            uuid: self.uuid,
            name: self.name,
            slug: self.slug,
            specs,
            archived: self.archived,
            last_heartbeat: self.last_heartbeat,
            created: self.created,
            modified: self.modified,
        })
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = runner_table)]
pub struct InsertRunner {
    pub uuid: RunnerUuid,
    pub name: ResourceName,
    pub slug: RunnerSlug,
    pub token_hash: TokenHash,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertRunner {
    pub fn new(name: ResourceName, slug: RunnerSlug, token_hash: TokenHash, now: DateTime) -> Self {
        Self {
            uuid: RunnerUuid::new(),
            name,
            slug,
            token_hash,
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
    pub token_hash: Option<TokenHash>,
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
