use bencher_json::{
    DateTime, JsonNewProjectKey, JsonProjectKey, JsonProjectKeyCreated, ProjectKey, ProjectKeyHash,
    ProjectKeyUuid, ProjectUuid, ResourceName, project::key::JsonUpdateProjectKey,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::{BencherResource, assert_parentage, bad_request_error, resource_not_found_err},
    macros::fn_get::{fn_get, fn_get_id, fn_get_uuid},
    model::user::{QueryUser, UserId},
    schema,
    schema::project_key as project_key_table,
};

use super::{ProjectId, QueryProject};

crate::macros::typed_id::typed_id!(ProjectKeyId);

#[derive(Debug, Clone, diesel::Queryable)]
pub struct QueryProjectKey {
    pub id: ProjectKeyId,
    pub uuid: ProjectKeyUuid,
    pub project_id: ProjectId,
    pub creator_id: Option<UserId>,
    pub name: ResourceName,
    pub key_hash: ProjectKeyHash,
    pub creation: DateTime,
    pub expiration: DateTime,
    pub revoked: Option<DateTime>,
}

impl QueryProjectKey {
    fn_get!(project_key, ProjectKeyId);
    fn_get_id!(project_key, ProjectKeyId, ProjectKeyUuid);
    fn_get_uuid!(project_key, ProjectKeyId, ProjectKeyUuid);

    pub fn get_project_key(
        conn: &mut DbConnection,
        project_id: ProjectId,
        uuid: ProjectKeyUuid,
    ) -> Result<Self, HttpError> {
        schema::project_key::table
            .filter(schema::project_key::project_id.eq(project_id))
            .filter(schema::project_key::uuid.eq(uuid))
            .first::<QueryProjectKey>(conn)
            .map_err(resource_not_found_err!(ProjectKey, (project_id, &uuid)))
    }

    pub fn from_hash(
        conn: &mut DbConnection,
        key_hash: &ProjectKeyHash,
        now: DateTime,
    ) -> diesel::QueryResult<Self> {
        schema::project_key::table
            .inner_join(schema::project::table)
            .filter(schema::project_key::key_hash.eq(key_hash.as_ref()))
            .filter(schema::project_key::revoked.is_null())
            .filter(schema::project_key::expiration.gt(now))
            .filter(schema::project::deleted.is_null())
            .select(schema::project_key::all_columns)
            .first::<QueryProjectKey>(conn)
    }

    pub fn revoke(
        conn: &mut DbConnection,
        key_id: ProjectKeyId,
        now: DateTime,
    ) -> diesel::QueryResult<usize> {
        diesel::update(
            schema::project_key::table
                .filter(schema::project_key::id.eq(key_id))
                .filter(schema::project_key::revoked.is_null()),
        )
        .set(schema::project_key::revoked.eq(now))
        .execute(conn)
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonProjectKey, HttpError> {
        let query_project = QueryProject::get(conn, self.project_id)?;
        let creator_uuid = self
            .creator_id
            .map(|id| QueryUser::get(conn, id).map(|u| u.uuid))
            .transpose()?;
        Ok(self.into_json_inner(query_project.uuid, creator_uuid))
    }

    pub fn into_json_for_project(
        self,
        query_project: &QueryProject,
        creator_uuid: Option<bencher_json::UserUuid>,
    ) -> JsonProjectKey {
        assert_parentage(
            BencherResource::Project,
            query_project.id,
            BencherResource::ProjectKey,
            self.project_id,
        );
        self.into_json_inner(query_project.uuid, creator_uuid)
    }

    fn into_json_inner(
        self,
        project_uuid: ProjectUuid,
        creator_uuid: Option<bencher_json::UserUuid>,
    ) -> JsonProjectKey {
        let Self {
            id: _,
            uuid,
            project_id: _,
            creator_id: _,
            name,
            key_hash: _,
            creation,
            expiration,
            revoked,
        } = self;
        JsonProjectKey {
            uuid,
            project: project_uuid,
            creator: creator_uuid,
            name,
            creation,
            expiration,
            revoked,
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = project_key_table)]
pub struct InsertProjectKey {
    pub uuid: ProjectKeyUuid,
    pub project_id: ProjectId,
    pub creator_id: Option<UserId>,
    pub name: ResourceName,
    pub key_hash: ProjectKeyHash,
    pub creation: DateTime,
    pub expiration: DateTime,
}

impl InsertProjectKey {
    pub fn from_json(
        project_id: ProjectId,
        creator_id: UserId,
        json_key: JsonNewProjectKey,
        now: DateTime,
    ) -> Result<(Self, ProjectKey), HttpError> {
        let JsonNewProjectKey { name, ttl } = json_key;

        let max_ttl = u32::MAX;
        let ttl = if let Some(ttl) = ttl {
            if ttl == 0 {
                return Err(bad_request_error("TTL must be greater than zero"));
            }
            if ttl > max_ttl {
                return Err(bad_request_error(format!(
                    "Requested TTL ({ttl}) is greater than max ({max_ttl})"
                )));
            }
            ttl
        } else {
            max_ttl
        };

        let key = ProjectKey::generate();
        let key_hash = ProjectKeyHash::from(&key);
        let creation = now;
        let expiration = creation + chrono::Duration::seconds(i64::from(ttl));

        Ok((
            Self {
                uuid: ProjectKeyUuid::new(),
                project_id,
                creator_id: Some(creator_id),
                name,
                key_hash,
                creation,
                expiration,
            },
            key,
        ))
    }

    pub fn into_json_created(
        self,
        project_uuid: ProjectUuid,
        key: ProjectKey,
    ) -> JsonProjectKeyCreated {
        let Self {
            uuid,
            project_id: _,
            creator_id: _,
            name,
            key_hash: _,
            creation,
            expiration,
        } = self;
        JsonProjectKeyCreated {
            uuid,
            project: project_uuid,
            name,
            key,
            creation,
            expiration,
        }
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = project_key_table)]
pub struct UpdateProjectKey {
    pub name: Option<ResourceName>,
}

impl From<JsonUpdateProjectKey> for UpdateProjectKey {
    fn from(update: JsonUpdateProjectKey) -> Self {
        let JsonUpdateProjectKey { name } = update;
        Self { name }
    }
}
