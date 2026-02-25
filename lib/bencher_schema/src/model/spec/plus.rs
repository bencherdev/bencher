use std::string::ToString as _;

use bencher_json::{
    Architecture, Cpu, DateTime, Disk, JsonNewSpec, JsonSpec, JsonUpdateSpec, Memory, ResourceName,
    SpecSlug, SpecUuid,
};
use diesel::{ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use super::SpecId;
use crate::{
    context::DbConnection,
    error::issue_error,
    macros::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
    },
    schema::{self, spec as spec_table},
};

#[derive(Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Selectable)]
#[diesel(table_name = spec_table)]
pub struct QuerySpec {
    pub id: SpecId,
    pub uuid: SpecUuid,
    pub name: ResourceName,
    pub slug: SpecSlug,
    pub architecture: Architecture,
    pub cpu: Cpu,
    pub memory: Memory,
    pub disk: Disk,
    pub network: bool,
    pub fallback: Option<DateTime>,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl QuerySpec {
    fn_get!(spec, SpecId);
    fn_get_id!(spec, SpecId, SpecUuid);
    fn_get_uuid!(spec, SpecId, SpecUuid);
    fn_from_uuid!(spec, SpecUuid, Spec);
    fn_eq_resource_id!(spec, SpecResourceId);
    fn_from_resource_id!(spec, Spec, SpecResourceId);

    pub fn into_json(self) -> JsonSpec {
        JsonSpec {
            uuid: self.uuid,
            name: self.name,
            slug: self.slug,
            architecture: self.architecture,
            cpu: self.cpu,
            memory: self.memory,
            disk: self.disk,
            network: self.network,
            fallback: self.fallback,
            created: self.created,
            modified: self.modified,
            archived: self.archived,
        }
    }

    /// Get the current fallback spec (where `fallback IS NOT NULL`).
    pub fn get_fallback(conn: &mut DbConnection) -> Result<Option<Self>, HttpError> {
        schema::spec::table
            .filter(schema::spec::fallback.is_not_null())
            .first::<Self>(conn)
            .optional()
            .map_err(|e| {
                let message = "Failed to query spec table for fallback";
                issue_error(message, message, e)
            })
    }

    /// Clear fallback on all specs (set `fallback = NULL` where IS NOT NULL).
    pub fn clear_fallback(conn: &mut DbConnection) -> Result<(), HttpError> {
        diesel::update(schema::spec::table.filter(schema::spec::fallback.is_not_null()))
            .set(schema::spec::fallback.eq(None::<DateTime>))
            .execute(conn)
            .map_err(|e| {
                let message = "Failed to clear fallback on spec table";
                issue_error(message, message, e)
            })?;
        Ok(())
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = spec_table)]
pub struct InsertSpec {
    pub uuid: SpecUuid,
    pub name: ResourceName,
    pub slug: SpecSlug,
    pub architecture: Architecture,
    pub cpu: Cpu,
    pub memory: Memory,
    pub disk: Disk,
    pub network: bool,
    pub fallback: Option<DateTime>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertSpec {
    pub fn new(conn: &mut DbConnection, json: &JsonNewSpec, now: DateTime) -> Self {
        let slug = ok_slug!(conn, &json.name, json.slug.clone(), spec, QuerySpec);
        Self {
            uuid: SpecUuid::new(),
            name: json.name.clone(),
            slug,
            architecture: json.architecture,
            cpu: json.cpu,
            memory: json.memory,
            disk: json.disk,
            network: json.network,
            fallback: json.fallback.then_some(now),
            created: now,
            modified: now,
        }
    }
}

#[derive(Debug, Default, diesel::AsChangeset)]
#[diesel(table_name = spec_table)]
pub struct UpdateSpec {
    pub name: Option<ResourceName>,
    pub slug: Option<SpecSlug>,
    pub fallback: Option<Option<DateTime>>,
    pub modified: Option<DateTime>,
    pub archived: Option<Option<DateTime>>,
}

impl UpdateSpec {
    pub fn new(update: JsonUpdateSpec, now: DateTime) -> Self {
        let JsonUpdateSpec {
            name,
            slug,
            fallback,
            archived,
        } = update;
        let is_archiving = archived == Some(true);
        let fallback = if is_archiving {
            Some(None) // Archiving always clears fallback
        } else {
            fallback.map(|f| f.then_some(now))
        };
        let archived = archived.map(|archived| archived.then_some(now));
        Self {
            name,
            slug,
            fallback,
            archived,
            modified: Some(now),
        }
    }
}
