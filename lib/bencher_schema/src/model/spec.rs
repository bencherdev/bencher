use std::string::ToString as _;

use bencher_json::{
    Architecture, Cpu, DateTime, Disk, JsonNewSpec, JsonSpec, JsonUpdateSpec, Memory, ResourceName,
    SpecSlug, SpecUuid,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    macros::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
    },
    schema::{self, spec as spec_table},
};

crate::macros::typed_id::typed_id!(SpecId);

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
    pub archived: Option<DateTime>,
    pub created: DateTime,
    pub modified: DateTime,
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
            archived: self.archived,
            created: self.created,
            modified: self.modified,
        }
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
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertSpec {
    pub fn new(conn: &mut DbConnection, json: &JsonNewSpec) -> Self {
        let now = DateTime::now();
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
    pub archived: Option<Option<DateTime>>,
    pub modified: Option<DateTime>,
}

impl From<JsonUpdateSpec> for UpdateSpec {
    fn from(update: JsonUpdateSpec) -> Self {
        let JsonUpdateSpec {
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
        }
    }
}
