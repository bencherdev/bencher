use bencher_json::{project::threshold::StatisticKind, Boundary, SampleSize, Window};
use diesel::{
    ExpressionMethods, JoinOnDsl, NullableExpressionMethods, QueryDsl, RunQueryDsl,
    SelectableHelper,
};

use crate::{
    context::DbConnection,
    model::project::{
        branch::BranchId,
        measure::MeasureId,
        testbed::TestbedId,
        threshold::{
            statistic::{QueryStatistic, StatisticId},
            ThresholdId,
        },
    },
    schema,
};

#[derive(Debug, Clone)]
pub struct MetricsThreshold {
    pub id: ThresholdId,
    pub statistic: MetricsStatistic,
}

#[derive(Debug, Clone)]
pub struct MetricsStatistic {
    pub id: StatisticId,
    pub test: StatisticKind,
    pub min_sample_size: Option<SampleSize>,
    pub max_sample_size: Option<SampleSize>,
    pub window: Option<Window>,
    pub lower_boundary: Option<Boundary>,
    pub upper_boundary: Option<Boundary>,
}

impl MetricsThreshold {
    pub fn new(
        conn: &mut DbConnection,
        branch_id: BranchId,
        testbed_id: TestbedId,
        measure_id: MeasureId,
    ) -> Option<Self> {
        schema::statistic::table
            .inner_join(
                schema::threshold::table.on(schema::statistic::id
                    .nullable()
                    .eq(schema::threshold::statistic_id)),
            )
            .filter(schema::threshold::branch_id.eq(branch_id))
            .filter(schema::threshold::testbed_id.eq(testbed_id))
            .filter(schema::threshold::measure_id.eq(measure_id))
            .select((schema::threshold::id, QueryStatistic::as_select()))
            .first::<(ThresholdId, QueryStatistic)>(conn)
            .map(|(threshold_id, query_statistic)| {
                let QueryStatistic {
                    id,
                    test,
                    min_sample_size,
                    max_sample_size,
                    window,
                    lower_boundary,
                    upper_boundary,
                    ..
                } = query_statistic;
                let statistic = MetricsStatistic {
                    id,
                    test,
                    min_sample_size,
                    max_sample_size,
                    window,
                    lower_boundary,
                    upper_boundary,
                };
                Self {
                    id: threshold_id,
                    statistic,
                }
            })
            .ok()
    }
}
