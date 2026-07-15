use bencher_json::TestbedUuid;
use bencher_rank::{Rank, RankGenerator};
use diesel::{BelongingToDsl as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::resource_not_found_err,
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
    fn get_all_for_plot(
        conn: &mut DbConnection,
        query_plot: &QueryPlot,
    ) -> Result<Vec<Self>, HttpError> {
        Self::belonging_to(query_plot)
            .order(plot_testbed_table::rank.asc())
            .load::<Self>(conn)
            .map_err(resource_not_found_err!(PlotTestbed, query_plot))
    }

    pub fn into_json_for_plot(
        conn: &mut DbConnection,
        query_plot: &QueryPlot,
    ) -> Result<Vec<TestbedUuid>, HttpError> {
        Ok(Self::get_all_for_plot(conn, query_plot)?
            .into_iter()
            .filter_map(|p| match QueryTestbed::get_uuid(conn, p.testbed_id) {
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
#[diesel(table_name = plot_testbed_table)]
pub struct InsertPlotTestbed {
    pub plot_id: PlotId,
    pub testbed_id: TestbedId,
    pub rank: Rank,
}

impl InsertPlotTestbed {
    /// Batch-insert pre-resolved testbed IDs into the `plot_testbed` table.
    pub fn from_resolved(
        conn: &mut DbConnection,
        plot_id: PlotId,
        testbed_ids: &[TestbedId],
    ) -> diesel::QueryResult<()> {
        let ranker = RankGenerator::new(testbed_ids.len());
        let inserts: Vec<Self> = testbed_ids
            .iter()
            .zip(ranker)
            .map(|(&testbed_id, rank)| Self {
                plot_id,
                testbed_id,
                rank,
            })
            .collect();
        if !inserts.is_empty() {
            diesel::insert_into(plot_testbed_table::table)
                .values(&inserts)
                .execute(conn)?;
        }
        Ok(())
    }
}
