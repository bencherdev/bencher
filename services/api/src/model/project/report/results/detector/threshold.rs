use bencher_json::{project::threshold::StatisticKind, Boundary, SampleSize};
use diesel::{
    ExpressionMethods, JoinOnDsl, NullableExpressionMethods, QueryDsl, RunQueryDsl,
    SelectableHelper,
};

use crate::{
    context::DbConnection,
    model::project::{
        branch::BranchId,
        metric_kind::MetricKindId,
        testbed::TestbedId,
        threshold::{
            statistic::{QueryStatistic, StatisticId},
            ThresholdId,
        },
    },
    schema,
    util::map_u32,
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
    pub window: Option<u32>,
    pub lower_boundary: Option<Boundary>,
    pub upper_boundary: Option<Boundary>,
}

impl MetricsThreshold {
    pub fn new(
        conn: &mut DbConnection,
        metric_kind_id: MetricKindId,
        branch_id: BranchId,
        testbed_id: TestbedId,
    ) -> Option<Self> {
        schema::statistic::table
            .inner_join(
                schema::threshold::table.on(schema::statistic::id
                    .nullable()
                    .eq(schema::threshold::statistic_id)),
            )
            .filter(schema::threshold::metric_kind_id.eq(metric_kind_id))
            .filter(schema::threshold::branch_id.eq(branch_id))
            .filter(schema::threshold::testbed_id.eq(testbed_id))
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
                    window: map_u32(window).ok()?,
                    lower_boundary,
                    upper_boundary,
                };
                Some(Self {
                    id: threshold_id,
                    statistic,
                })
            })
            .ok()
            .flatten()
    }
}
