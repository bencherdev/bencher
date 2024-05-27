use bencher_json::TestbedUuid;
use bencher_rank::{Rank, RankGenerator};
use diesel::{BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{resource_conflict_err, resource_not_found_err},
    model::project::testbed::{QueryTestbed, TestbedId},
    schema::plot_testbed as plot_testbed_table,
};

use super::{PlotId, QueryPlot};

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = plot_testbed_table)]
#[diesel(primary_key(plot_id, testbed_id))]
#[diesel(belongs_to(QueryPlot, foreign_key = plot_id))]
pub struct QueryPlotTestbed {
    pub plot_id: PlotId,
    pub testbed_id: TestbedId,
    pub rank: Rank,
}
impl QueryPlotTestbed {
    pub fn get_all_for_plot(
        conn: &mut DbConnection,
        query_plot: &QueryPlot,
    ) -> Result<Vec<Self>, HttpError> {
        Self::belonging_to(query_plot)
            .order(plot_testbed_table::rank.asc())
            .load::<Self>(conn)
            .map_err(resource_not_found_err!(PlotTestbed, query_plot))
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = plot_testbed_table)]
pub struct InsertPlotTestbed {
    pub plot_id: PlotId,
    pub testbed_id: TestbedId,
    pub rank: Rank,
}

impl InsertPlotTestbed {
    pub async fn from_json(
        context: &ApiContext,
        plot_id: PlotId,
        testbeds: Vec<TestbedUuid>,
    ) -> Result<(), HttpError> {
        let ranker = RankGenerator::new(testbeds.len());
        for (uuid, rank) in testbeds.into_iter().zip(ranker) {
            let testbed_id = QueryTestbed::get_id(conn_lock!(context), uuid)?;
            let insert_plot_testbed = Self {
                plot_id,
                testbed_id,
                rank,
            };
            diesel::insert_into(plot_testbed_table::table)
                .values(&insert_plot_testbed)
                .execute(conn_lock!(context))
                .map_err(resource_conflict_err!(PlotTestbed, insert_plot_testbed))?;
        }
        Ok(())
    }
}
