use std::string::ToString as _;

use bencher_json::{
    DateTime, JsonRunner, JsonUpdateRunner, ResourceName, RunnerSlug, SpecUuid,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

pub use bencher_json::{JobStatus, JobUuid, RunnerUuid};

use crate::{
    context::DbConnection,
    macros::{
        fn_get::{fn_get, fn_get_id, fn_get_uuid},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
    },
    model::spec::QuerySpec,
    resource_not_found_err,
    schema::{self, runner as runner_table},
};

pub mod job;
pub mod runner_spec;
mod source_ip;
mod token_hash;

pub use job::{
    InsertJob, JobId, QueryJob, UpdateJob, recover_orphaned_claimed_jobs, spawn_heartbeat_timeout,
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
    pub archived: Option<DateTime>,
    pub last_heartbeat: Option<DateTime>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryRunner {
    fn_get!(runner, RunnerId);
    fn_get_id!(runner, RunnerId, RunnerUuid);
    fn_get_uuid!(runner, RunnerId, RunnerUuid);
    fn_eq_resource_id!(runner, RunnerResourceId);
    fn_from_resource_id!(runner, Runner, RunnerResourceId);

    pub fn from_uuid(conn: &mut DbConnection, uuid: RunnerUuid) -> Result<Self, HttpError> {
        schema::runner::table
            .filter(schema::runner::uuid.eq(uuid))
            .first(conn)
            .map_err(resource_not_found_err!(Runner, uuid))
    }

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
    pub fn new(name: ResourceName, slug: RunnerSlug, token_hash: TokenHash) -> Self {
        let now = DateTime::now();
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
    pub archived: Option<Option<DateTime>>,
    pub last_heartbeat: Option<Option<DateTime>>,
    pub modified: Option<DateTime>,
}

impl From<JsonUpdateRunner> for UpdateRunner {
    fn from(update: JsonUpdateRunner) -> Self {
        let JsonUpdateRunner {
            name,
            slug,
            archived,
        } = update;
        let modified = DateTime::now();
        let archived = archived.map(|archived| archived.then_some(modified));
        Self {
            name,
            slug,
            archived,
            modified: Some(modified),
            ..Default::default()
        }
    }
}
