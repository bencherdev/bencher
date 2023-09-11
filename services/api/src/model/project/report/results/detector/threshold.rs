use bencher_json::Boundary;
use diesel::{ExpressionMethods, JoinOnDsl, NullableExpressionMethods, QueryDsl, RunQueryDsl};

use crate::{
    context::DbConnection,
    model::project::{
        branch::BranchId,
        metric_kind::MetricKindId,
        threshold::statistic::{map_boundary, StatisticKind},
    },
    schema,
    util::map_u32,
};

#[derive(Debug, Clone)]
pub struct MetricsThreshold {
    pub id: i32,
    pub statistic: MetricsStatistic,
}

#[derive(Debug, Clone)]
pub struct MetricsStatistic {
    pub id: i32,
    pub test: StatisticKind,
    pub min_sample_size: Option<u32>,
    pub max_sample_size: Option<u32>,
    pub window: Option<u32>,
    pub lower_boundary: Option<Boundary>,
    pub upper_boundary: Option<Boundary>,
}

impl MetricsThreshold {
    pub fn new(
        conn: &mut DbConnection,
        metric_kind_id: MetricKindId,
        branch_id: BranchId,
        testbed_id: i32,
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
            .select((
                schema::threshold::id,
                schema::statistic::id,
                schema::statistic::test,
                schema::statistic::min_sample_size,
                schema::statistic::max_sample_size,
                schema::statistic::window,
                schema::statistic::lower_boundary,
                schema::statistic::upper_boundary,
            ))
            .first::<(
                i32,
                i32,
                i32,
                Option<i64>,
                Option<i64>,
                Option<i64>,
                Option<f64>,
                Option<f64>,
            )>(conn)
            .map(
                |(
                    threshold_id,
                    statistic_id,
                    test,
                    min_sample_size,
                    max_sample_size,
                    window,
                    lower_boundary,
                    upper_boundary,
                )| {
                    let statistic = MetricsStatistic {
                        id: statistic_id,
                        test: StatisticKind::try_from(test).ok()?,
                        min_sample_size: map_u32(min_sample_size).ok()?,
                        max_sample_size: map_u32(max_sample_size).ok()?,
                        window: map_u32(window).ok()?,
                        lower_boundary: map_boundary(lower_boundary).ok()?,
                        upper_boundary: map_boundary(upper_boundary).ok()?,
                    };
                    Some(Self {
                        id: threshold_id,
                        statistic,
                    })
                },
            )
            .ok()
            .flatten()
    }
}
