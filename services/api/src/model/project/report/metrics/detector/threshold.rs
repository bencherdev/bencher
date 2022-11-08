use diesel::{
    expression_methods::BoolExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl, SqliteConnection,
};

use crate::{
    diesel::ExpressionMethods, model::project::threshold::statistic::QueryStatistic, schema,
};

#[derive(Debug, Clone)]
pub struct MetricsThreshold {
    pub id: i32,
    pub statistic: QueryStatistic,
}

impl MetricsThreshold {
    pub fn new(
        conn: &mut SqliteConnection,
        branch_id: i32,
        testbed_id: i32,
        metric_kind_id: i32,
    ) -> Option<Self> {
        schema::statistic::table
            .inner_join(
                schema::threshold::table
                    .on(schema::statistic::id.eq(schema::threshold::statistic_id)),
            )
            .filter(
                schema::threshold::branch_id
                    .eq(branch_id)
                    .and(schema::threshold::testbed_id.eq(testbed_id))
                    .and(schema::threshold::metric_kind_id.eq(metric_kind_id)),
            )
            .select((
                schema::threshold::id,
                schema::statistic::id,
                schema::statistic::uuid,
                schema::statistic::test,
                schema::statistic::min_sample_size,
                schema::statistic::max_sample_size,
                schema::statistic::window,
                schema::statistic::left_side,
                schema::statistic::right_side,
            ))
            .first::<(
                i32,
                i32,
                String,
                i32,
                Option<i64>,
                Option<i64>,
                Option<i64>,
                Option<f32>,
                Option<f32>,
            )>(conn)
            .map(
                |(
                    threshold_id,
                    statistic_id,
                    uuid,
                    test,
                    min_sample_size,
                    max_sample_size,
                    window,
                    left_side,
                    right_side,
                )| {
                    let statistic = QueryStatistic {
                        id: statistic_id,
                        uuid,
                        test,
                        min_sample_size,
                        max_sample_size,
                        window,
                        left_side,
                        right_side,
                    };
                    Self {
                        id: threshold_id,
                        statistic,
                    }
                },
            )
            .ok()
    }
}
