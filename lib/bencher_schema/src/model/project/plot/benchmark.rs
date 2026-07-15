use bencher_json::BenchmarkUuid;
use bencher_rank::{Rank, RankGenerator};
use diesel::{BelongingToDsl as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::resource_not_found_err,
    model::project::benchmark::{BenchmarkId, QueryBenchmark},
    schema::plot_benchmark as plot_benchmark_table,
};

use super::{PlotId, QueryPlot};

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = plot_benchmark_table)]
#[diesel(primary_key(plot_id, benchmark_id))]
#[diesel(belongs_to(QueryPlot, foreign_key = plot_id))]
pub struct QueryPlotBenchmark {
    pub plot_id: PlotId,
    pub benchmark_id: BenchmarkId,
    pub rank: Rank,
}
impl QueryPlotBenchmark {
    fn get_all_for_plot(
        conn: &mut DbConnection,
        query_plot: &QueryPlot,
    ) -> Result<Vec<Self>, HttpError> {
        Self::belonging_to(query_plot)
            .order(plot_benchmark_table::rank.asc())
            .load::<Self>(conn)
            .map_err(resource_not_found_err!(PlotBenchmark, query_plot))
    }

    pub fn into_json_for_plot(
        conn: &mut DbConnection,
        query_plot: &QueryPlot,
    ) -> Result<Vec<BenchmarkUuid>, HttpError> {
        Ok(Self::get_all_for_plot(conn, query_plot)?
            .into_iter()
            .filter_map(|p| match QueryBenchmark::get_uuid(conn, p.benchmark_id) {
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
#[diesel(table_name = plot_benchmark_table)]
pub struct InsertPlotBenchmark {
    pub plot_id: PlotId,
    pub benchmark_id: BenchmarkId,
    pub rank: Rank,
}

impl InsertPlotBenchmark {
    /// Batch-insert pre-resolved benchmark IDs into the `plot_benchmark` table.
    pub fn from_resolved(
        conn: &mut DbConnection,
        plot_id: PlotId,
        benchmark_ids: &[BenchmarkId],
    ) -> diesel::QueryResult<()> {
        let ranker = RankGenerator::new(benchmark_ids.len());
        let inserts: Vec<Self> = benchmark_ids
            .iter()
            .zip(ranker)
            .map(|(&benchmark_id, rank)| Self {
                plot_id,
                benchmark_id,
                rank,
            })
            .collect();
        if !inserts.is_empty() {
            diesel::insert_into(plot_benchmark_table::table)
                .values(&inserts)
                .execute(conn)?;
        }
        Ok(())
    }
}
