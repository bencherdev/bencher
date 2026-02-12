use bencher_json::{
    Architecture, Cpu, DateTime, Disk, JsonNewSpec, JsonSpec, JsonUpdateSpec, Memory, SpecUuid,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    resource_not_found_err,
    schema::{self, spec as spec_table},
};

crate::macros::typed_id::typed_id!(SpecId);

#[derive(Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Selectable)]
#[diesel(table_name = spec_table)]
pub struct QuerySpec {
    pub id: SpecId,
    pub uuid: SpecUuid,
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
    pub fn get(conn: &mut DbConnection, id: SpecId) -> Result<Self, HttpError> {
        schema::spec::table
            .filter(schema::spec::id.eq(id))
            .first(conn)
            .map_err(resource_not_found_err!(Spec, id))
    }

    pub fn from_uuid(conn: &mut DbConnection, uuid: SpecUuid) -> Result<Self, HttpError> {
        schema::spec::table
            .filter(schema::spec::uuid.eq(uuid))
            .first(conn)
            .map_err(resource_not_found_err!(Spec, uuid))
    }

    pub fn into_json(self) -> JsonSpec {
        JsonSpec {
            uuid: self.uuid,
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
    pub architecture: Architecture,
    pub cpu: Cpu,
    pub memory: Memory,
    pub disk: Disk,
    pub network: bool,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertSpec {
    pub fn new(json: &JsonNewSpec) -> Self {
        let now = DateTime::now();
        Self {
            uuid: SpecUuid::new(),
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
    pub archived: Option<Option<DateTime>>,
    pub modified: Option<DateTime>,
}

impl From<JsonUpdateSpec> for UpdateSpec {
    fn from(update: JsonUpdateSpec) -> Self {
        let modified = DateTime::now();
        let archived = update.archived.map(|archived| archived.then_some(modified));
        Self {
            archived,
            modified: Some(modified),
        }
    }
}
