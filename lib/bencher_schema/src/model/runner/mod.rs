use bencher_json::{DateTime, JsonRunner, ResourceName, RunnerResourceId, Slug};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

pub use bencher_json::{JobStatus, JobUuid, RunnerState, RunnerUuid};

use crate::{
    context::DbConnection,
    resource_not_found_err,
    schema::{self, runner as runner_table},
};

pub mod job;

pub use job::{InsertJob, JobId, QueryJob, UpdateJob, spawn_heartbeat_timeout};

crate::macros::typed_id::typed_id!(RunnerId);

#[derive(Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Selectable)]
#[diesel(table_name = runner_table)]
pub struct QueryRunner {
    pub id: RunnerId,
    pub uuid: RunnerUuid,
    pub name: ResourceName,
    pub slug: Slug,
    pub token_hash: String,
    pub state: RunnerState,
    pub locked: Option<DateTime>,
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

    pub fn is_locked(&self) -> bool {
        self.locked.is_some()
    }

    pub fn is_archived(&self) -> bool {
        self.archived.is_some()
    }

    pub fn into_json(self) -> JsonRunner {
        JsonRunner {
            uuid: self.uuid,
            name: self.name,
            slug: self.slug,
            state: self.state,
            locked: self.locked,
            archived: self.archived,
            last_heartbeat: self.last_heartbeat,
            created: self.created,
            modified: self.modified,
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = runner_table)]
pub struct InsertRunner {
    pub uuid: RunnerUuid,
    pub name: ResourceName,
    pub slug: Slug,
    pub token_hash: String,
    pub state: RunnerState,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertRunner {
    pub fn new(name: ResourceName, slug: Slug, token_hash: String) -> Self {
        let now = DateTime::now();
        Self {
            uuid: RunnerUuid::new(),
            name,
            slug,
            token_hash,
            state: RunnerState::default(),
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
    pub token_hash: Option<String>,
    pub state: Option<RunnerState>,
    pub locked: Option<Option<DateTime>>,
    pub archived: Option<Option<DateTime>>,
    pub last_heartbeat: Option<Option<DateTime>>,
    pub modified: Option<DateTime>,
}
