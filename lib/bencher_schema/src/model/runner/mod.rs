use bencher_json::{
    DateTime, JsonRunner, JsonUpdateRunner, ResourceName, RunnerResourceId, Slug, SpecUuid,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

pub use bencher_json::{JobStatus, JobUuid, RunnerUuid};

use crate::{
    context::DbConnection,
    resource_not_found_err,
    schema::{self, runner as runner_table},
};

pub mod job;
pub mod runner_spec;
mod source_ip;
pub mod spec;
mod token_hash;

pub use job::{InsertJob, JobId, QueryJob, UpdateJob, spawn_heartbeat_timeout};
pub use runner_spec::{InsertRunnerSpec, QueryRunnerSpec, RunnerSpecId};
pub use source_ip::SourceIp;
pub use spec::{InsertSpec, QuerySpec, SpecId, UpdateSpec};
pub use token_hash::TokenHash;

crate::macros::typed_id::typed_id!(RunnerId);

#[derive(Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Selectable)]
#[diesel(table_name = runner_table)]
pub struct QueryRunner {
    pub id: RunnerId,
    pub uuid: RunnerUuid,
    pub name: ResourceName,
    pub slug: Slug,
    pub token_hash: TokenHash,
    pub archived: Option<DateTime>,
    pub last_heartbeat: Option<DateTime>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryRunner {
    pub fn get(conn: &mut DbConnection, id: RunnerId) -> Result<Self, HttpError> {
        schema::runner::table
            .filter(schema::runner::id.eq(id))
            .first(conn)
            .map_err(resource_not_found_err!(Runner, id))
    }

    pub fn from_uuid(conn: &mut DbConnection, uuid: RunnerUuid) -> Result<Self, HttpError> {
        schema::runner::table
            .filter(schema::runner::uuid.eq(uuid))
            .first(conn)
            .map_err(resource_not_found_err!(Runner, uuid))
    }

    pub fn from_slug(conn: &mut DbConnection, slug: &Slug) -> Result<Self, HttpError> {
        schema::runner::table
            .filter(schema::runner::slug.eq(slug))
            .first(conn)
            .map_err(resource_not_found_err!(Runner, slug))
    }

    pub fn from_resource_id(
        conn: &mut DbConnection,
        resource_id: &RunnerResourceId,
    ) -> Result<Self, HttpError> {
        match resource_id {
            RunnerResourceId::Uuid(uuid) => Self::from_uuid(conn, *uuid),
            RunnerResourceId::Slug(slug) => Self::from_slug(conn, slug.as_ref()),
        }
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
    pub slug: Slug,
    pub token_hash: TokenHash,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertRunner {
    pub fn new(name: ResourceName, slug: Slug, token_hash: TokenHash) -> Self {
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
    pub slug: Option<Slug>,
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
