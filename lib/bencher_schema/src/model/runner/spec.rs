use bencher_json::{DateTime, JsonNewSpec, JsonSpec, JsonUpdateSpec, SpecUuid};
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
    pub cpu: i32,
    pub memory: i64,
    pub disk: i64,
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

    pub fn into_json(self) -> Result<JsonSpec, HttpError> {
        #[expect(
            clippy::cast_sign_loss,
            reason = "CPU stored as i32 but is always non-negative"
        )]
        let cpu = (self.cpu as u32).try_into().map_err(|e| {
            crate::error::issue_error("Invalid CPU value", "CPU value in database is invalid", e)
        })?;
        #[expect(
            clippy::cast_sign_loss,
            reason = "memory stored as i64 but is always non-negative"
        )]
        let memory = (self.memory as u64).try_into().map_err(|e| {
            crate::error::issue_error(
                "Invalid memory value",
                "Memory value in database is invalid",
                e,
            )
        })?;
        #[expect(
            clippy::cast_sign_loss,
            reason = "disk stored as i64 but is always non-negative"
        )]
        let disk = (self.disk as u64).try_into().map_err(|e| {
            crate::error::issue_error("Invalid disk value", "Disk value in database is invalid", e)
        })?;
        Ok(JsonSpec {
            uuid: self.uuid,
            cpu,
            memory,
            disk,
            network: self.network,
            archived: self.archived,
            created: self.created,
            modified: self.modified,
        })
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = spec_table)]
pub struct InsertSpec {
    pub uuid: SpecUuid,
    pub cpu: i32,
    pub memory: i64,
    pub disk: i64,
    pub network: bool,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertSpec {
    pub fn new(json: &JsonNewSpec) -> Self {
        let now = DateTime::now();
        #[expect(clippy::cast_possible_wrap, reason = "CPU count fits in i32 (max 256)")]
        let cpu = u32::from(json.cpu) as i32;
        #[expect(
            clippy::cast_possible_wrap,
            reason = "memory in bytes fits in i64 (max ~9.2 EB)"
        )]
        let memory = u64::from(json.memory) as i64;
        #[expect(
            clippy::cast_possible_wrap,
            reason = "disk in bytes fits in i64 (max ~9.2 EB)"
        )]
        let disk = u64::from(json.disk) as i64;
        Self {
            uuid: SpecUuid::new(),
            cpu,
            memory,
            disk,
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
