use bencher_json::BranchUuid;
use bencher_rank::Rank;
use diesel::{BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::resource_not_found_err,
    model::project::branch::{BranchId, QueryBranch},
    schema::plot_branch as plot_branch_table,
};

use super::{PlotId, QueryPlot};

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = plot_branch_table)]
#[diesel(primary_key(plot_id, branch_id))]
#[diesel(belongs_to(QueryPlot, foreign_key = plot_id))]
pub struct QueryPlotBranch {
    pub plot_id: PlotId,
    pub branch_id: BranchId,
    pub rank: Rank,
}
impl QueryPlotBranch {
    pub fn get_all_for_plot(
        conn: &mut DbConnection,
        query_plot: &QueryPlot,
    ) -> Result<Vec<Self>, HttpError> {
        Self::belonging_to(query_plot)
            .order(plot_branch_table::rank.asc())
            .load::<Self>(conn)
            .map_err(resource_not_found_err!(PlotBranch, query_plot))
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = plot_branch_table)]
pub struct InsertPlotBranch {
    pub plot_id: PlotId,
    pub branch_id: BranchId,
    pub rank: Rank,
}

impl InsertPlotBranch {
    pub fn from_json(
        conn: &mut DbConnection,
        plot_id: PlotId,
        branch_uuid: BranchUuid,
        rank: Rank,
    ) -> Result<Self, HttpError> {
        Ok(Self {
            plot_id,
            branch_id: QueryBranch::get_id(conn, branch_uuid)?,
            rank,
        })
    }
}
