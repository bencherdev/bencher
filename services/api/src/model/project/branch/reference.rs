use bencher_json::{
    project::reference::{JsonVersion, VersionNumber},
    BranchUuid, DateTime, GitHash, JsonReference, JsonStartPoint, ReferenceUuid,
};
use diesel::{ExpressionMethods, JoinOnDsl, NullableExpressionMethods, QueryDsl, RunQueryDsl};

use dropshot::HttpError;

use super::{reference_version::ReferenceVersionId, BranchId, QueryBranch};
use crate::{
    context::DbConnection,
    error::resource_not_found_err,
    schema::{self, reference as reference_table},
};

crate::util::typed_id::typed_id!(ReferenceId);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = reference_table)]
#[diesel(belongs_to(QueryBranch, foreign_key = branch_id))]
pub struct QueryReference {
    pub id: ReferenceId,
    pub uuid: ReferenceUuid,
    pub branch_id: BranchId,
    pub start_point_id: Option<ReferenceVersionId>,
    pub created: DateTime,
    pub replaced: Option<DateTime>,
}

impl QueryReference {
    pub fn get_json(
        conn: &mut DbConnection,
        reference_id: ReferenceId,
    ) -> Result<JsonReference, HttpError> {
        let (reference_uuid, branch_uuid, start_point_id, created, replaced) =
            schema::reference::table
                .inner_join(
                    schema::branch::table.on(schema::branch::id.eq(schema::reference::branch_id)),
                )
                .filter(schema::reference::id.eq(reference_id))
                .select((
                    schema::reference::uuid,
                    schema::branch::uuid,
                    schema::reference::start_point_id.nullable(),
                    schema::reference::created,
                    schema::reference::replaced.nullable(),
                ))
                .first::<(
                    ReferenceUuid,
                    BranchUuid,
                    Option<ReferenceVersionId>,
                    DateTime,
                    Option<DateTime>,
                )>(conn)
                .map_err(resource_not_found_err!(Reference, reference_id))?;

        let start_point = if let Some(start_point_id) = start_point_id {
            let (branch, reference, number, hash) = schema::reference_version::table
                .inner_join(
                    schema::reference::table
                        .on(schema::reference::id.eq(schema::reference_version::reference_id))
                        .inner_join(
                            schema::branch::table
                                .on(schema::branch::id.eq(schema::reference::branch_id)),
                        ),
                )
                .inner_join(schema::version::table)
                .filter(schema::reference_version::id.eq(start_point_id))
                .select((
                    schema::branch::uuid,
                    schema::reference::uuid,
                    schema::version::number,
                    schema::version::hash.nullable(),
                ))
                .first::<(BranchUuid, ReferenceUuid, VersionNumber, Option<GitHash>)>(conn)
                .map_err(resource_not_found_err!(ReferenceVersion, start_point_id))?;

            Some(JsonStartPoint {
                branch,
                reference,
                version: JsonVersion { number, hash },
            })
        } else {
            None
        };

        Ok(JsonReference {
            uuid: reference_uuid,
            branch: branch_uuid,
            start_point,
            created,
            replaced,
        })
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = reference_table)]
pub struct InsertReference {
    pub uuid: ReferenceUuid,
    pub branch_id: BranchId,
    pub start_point_id: Option<ReferenceVersionId>,
    pub created: DateTime,
    pub replaced: Option<DateTime>,
}

impl InsertReference {
    pub fn new(branch_id: BranchId, start_point_id: Option<ReferenceVersionId>) -> Self {
        Self {
            uuid: ReferenceUuid::new(),
            branch_id,
            start_point_id,
            created: DateTime::now(),
            replaced: None,
        }
    }
}
