use bencher_json::JsonStartPoint;
use dropshot::HttpError;

use crate::{
    context::DbConnection, schema::branch_version as branch_version_table, util::fn_get::fn_get,
};

use super::{
    branch::{BranchId, QueryBranch},
    version::{QueryVersion, VersionId},
};

crate::util::typed_id::typed_id!(BranchVersionId);

#[derive(Debug, diesel::Queryable)]
pub struct QueryBranchVersion {
    pub id: BranchVersionId,
    pub branch_id: BranchId,
    pub version_id: VersionId,
}

impl QueryBranchVersion {
    fn_get!(branch_version, BranchVersionId);

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonStartPoint, HttpError> {
        Ok(JsonStartPoint {
            branch: QueryBranch::get_uuid(conn, self.branch_id)?,
            version: QueryVersion::get(conn, self.version_id)?.into_json(),
        })
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = branch_version_table)]
pub struct InsertBranchVersion {
    pub branch_id: BranchId,
    pub version_id: VersionId,
}
