use bencher_json::BranchUuid;
use bencher_rank::{Rank, RankGenerator};
use diesel::{BelongingToDsl as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
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
    fn get_all_for_plot(
        conn: &mut DbConnection,
        query_plot: &QueryPlot,
    ) -> Result<Vec<Self>, HttpError> {
        Self::belonging_to(query_plot)
            .order(plot_branch_table::rank.asc())
            .load::<Self>(conn)
            .map_err(resource_not_found_err!(PlotBranch, query_plot))
    }

    pub fn into_json_for_plot(
        conn: &mut DbConnection,
        query_plot: &QueryPlot,
    ) -> Result<Vec<BranchUuid>, HttpError> {
        Ok(Self::get_all_for_plot(conn, query_plot)?
            .into_iter()
            .filter_map(|p| match QueryBranch::get_uuid(conn, p.branch_id) {
                Ok(uuid) => Some(uuid),
                Err(err) => {
                    debug_assert!(false, "{err}");
                    #[cfg(feature = "sentry")]
                    sentry::capture_error(&err);
                    None
                },
            })
            .collect())
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
    /// Batch-insert pre-resolved branch IDs into the `plot_branch` table.
    pub fn from_resolved(
        conn: &mut DbConnection,
        plot_id: PlotId,
        branch_ids: &[BranchId],
    ) -> diesel::QueryResult<()> {
        let ranker = RankGenerator::new(branch_ids.len());
        let inserts: Vec<Self> = branch_ids
            .iter()
            .zip(ranker)
            .map(|(&branch_id, rank)| Self {
                plot_id,
                branch_id,
                rank,
            })
            .collect();
        if !inserts.is_empty() {
            diesel::insert_into(plot_branch_table::table)
                .values(&inserts)
                .execute(conn)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, diesel::AsChangeset)]
#[diesel(table_name = plot_branch_table)]
pub struct UpdatePlotBranch {
    pub branch_id: BranchId,
}

impl From<BranchId> for UpdatePlotBranch {
    fn from(branch_id: BranchId) -> Self {
        Self { branch_id }
    }
}
