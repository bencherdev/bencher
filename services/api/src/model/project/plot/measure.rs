use bencher_json::MeasureUuid;
use bencher_rank::{Rank, RankGenerator};
use diesel::{BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{resource_conflict_err, resource_not_found_err},
    model::project::measure::{MeasureId, QueryMeasure},
    schema::plot_measure as plot_measure_table,
};

use super::{PlotId, QueryPlot};

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = plot_measure_table)]
#[diesel(primary_key(plot_id, measure_id))]
#[diesel(belongs_to(QueryPlot, foreign_key = plot_id))]
pub struct QueryPlotMeasure {
    pub plot_id: PlotId,
    pub measure_id: MeasureId,
    pub rank: Rank,
}
impl QueryPlotMeasure {
    pub fn get_all_for_plot(
        conn: &mut DbConnection,
        query_plot: &QueryPlot,
    ) -> Result<Vec<Self>, HttpError> {
        Self::belonging_to(query_plot)
            .order(plot_measure_table::rank.asc())
            .load::<Self>(conn)
            .map_err(resource_not_found_err!(PlotMeasure, query_plot))
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = plot_measure_table)]
pub struct InsertPlotMeasure {
    pub plot_id: PlotId,
    pub measure_id: MeasureId,
    pub rank: Rank,
}

impl InsertPlotMeasure {
    pub async fn from_json(
        context: &ApiContext,
        plot_id: PlotId,
        measures: Vec<MeasureUuid>,
    ) -> Result<(), HttpError> {
        let ranker = RankGenerator::new(measures.len());
        for (uuid, rank) in measures.into_iter().zip(ranker) {
            let measure_id = QueryMeasure::get_id(conn_lock!(context), uuid)?;
            let insert_plot_measure = Self {
                plot_id,
                measure_id,
                rank,
            };
            diesel::insert_into(plot_measure_table::table)
                .values(&insert_plot_measure)
                .execute(conn_lock!(context))
                .map_err(resource_conflict_err!(PlotMeasure, insert_plot_measure))?;
        }
        Ok(())
    }
}
