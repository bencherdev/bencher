use bencher_json::MeasureUuid;
use bencher_rank::{Rank, RankGenerator};
use diesel::{BelongingToDsl as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::resource_not_found_err,
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
    fn get_all_for_plot(
        conn: &mut DbConnection,
        query_plot: &QueryPlot,
    ) -> Result<Vec<Self>, HttpError> {
        Self::belonging_to(query_plot)
            .order(plot_measure_table::rank.asc())
            .load::<Self>(conn)
            .map_err(resource_not_found_err!(PlotMeasure, query_plot))
    }

    pub fn into_json_for_plot(
        conn: &mut DbConnection,
        query_plot: &QueryPlot,
    ) -> Result<Vec<MeasureUuid>, HttpError> {
        Ok(Self::get_all_for_plot(conn, query_plot)?
            .into_iter()
            .filter_map(|p| match QueryMeasure::get_uuid(conn, p.measure_id) {
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
#[diesel(table_name = plot_measure_table)]
pub struct InsertPlotMeasure {
    pub plot_id: PlotId,
    pub measure_id: MeasureId,
    pub rank: Rank,
}

impl InsertPlotMeasure {
    /// Batch-insert pre-resolved measure IDs into the `plot_measure` table.
    pub fn from_resolved(
        conn: &mut DbConnection,
        plot_id: PlotId,
        measure_ids: &[MeasureId],
    ) -> diesel::QueryResult<()> {
        let ranker = RankGenerator::new(measure_ids.len());
        let inserts: Vec<Self> = measure_ids
            .iter()
            .zip(ranker)
            .map(|(&measure_id, rank)| Self {
                plot_id,
                measure_id,
                rank,
            })
            .collect();
        if !inserts.is_empty() {
            diesel::insert_into(plot_measure_table::table)
                .values(&inserts)
                .execute(conn)?;
        }
        Ok(())
    }
}
