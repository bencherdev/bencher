use bencher_json::BranchUuid;
use bencher_rank::{Rank, RankGenerator};
use diesel::{BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{resource_conflict_err, resource_not_found_err},
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
    pub async fn from_json(
        context: &ApiContext,
        plot_id: PlotId,
        branches: Vec<BranchUuid>,
    ) -> Result<(), HttpError> {
        let ranker = RankGenerator::new(branches.len());
        for (uuid, rank) in branches.into_iter().zip(ranker) {
            let branch_id = QueryBranch::get_id(conn_lock!(context), uuid)?;
            let insert_plot_branch = Self {
                plot_id,
                branch_id,
                rank,
            };
            diesel::insert_into(plot_branch_table::table)
                .values(&insert_plot_branch)
                .execute(conn_lock!(context))
                .map_err(resource_conflict_err!(PlotBranch, insert_plot_branch))?;
        }
        Ok(())
    }
}
