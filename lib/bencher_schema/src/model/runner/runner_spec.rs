use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use super::RunnerId;
use crate::model::spec::SpecId;
use crate::{
    context::DbConnection,
    resource_not_found_err,
    schema::{self, runner_spec as runner_spec_table},
};

crate::macros::typed_id::typed_id!(RunnerSpecId);

#[derive(Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Selectable)]
#[diesel(table_name = runner_spec_table)]
pub struct QueryRunnerSpec {
    pub id: RunnerSpecId,
    pub runner_id: RunnerId,
    pub spec_id: SpecId,
}

impl QueryRunnerSpec {
    pub fn spec_ids_for_runner(
        conn: &mut DbConnection,
        runner_id: RunnerId,
    ) -> Result<Vec<SpecId>, HttpError> {
        schema::runner_spec::table
            .filter(schema::runner_spec::runner_id.eq(runner_id))
            .select(schema::runner_spec::spec_id)
            .load(conn)
            .map_err(resource_not_found_err!(RunnerSpec, runner_id))
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = runner_spec_table)]
pub struct InsertRunnerSpec {
    pub runner_id: RunnerId,
    pub spec_id: SpecId,
}
