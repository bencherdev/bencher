use bencher_json::BenchmarkUuid;
use bencher_rank::{Rank, RankGenerator};
use diesel::{BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{resource_conflict_err, resource_not_found_err},
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
    pub async fn from_json(
        context: &ApiContext,
        plot_id: PlotId,
        benchmarks: Vec<BenchmarkUuid>,
    ) -> Result<(), HttpError> {
        let ranker = RankGenerator::new(benchmarks.len());
        for (uuid, rank) in benchmarks.into_iter().zip(ranker) {
            let benchmark_id = QueryBenchmark::get_id(conn_lock!(context), uuid)?;
            let insert_plot_benchmark = Self {
                plot_id,
                benchmark_id,
                rank,
            };
            diesel::insert_into(plot_benchmark_table::table)
                .values(&insert_plot_benchmark)
                .execute(conn_lock!(context))
                .map_err(resource_conflict_err!(PlotBenchmark, insert_plot_benchmark))?;
        }
        Ok(())
    }
}
