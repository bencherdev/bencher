use bencher_json::{DateTime, ReferenceUuid};

use super::{reference_version::ReferenceVersionId, BranchId, QueryBranch};
use crate::schema::reference as reference_table;

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

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = reference_table)]
pub struct InsertReference {
    pub uuid: ReferenceUuid,
    pub branch_id: BranchId,
    pub start_point_id: Option<ReferenceVersionId>,
    pub created: DateTime,
    pub replaced: Option<DateTime>,
}
