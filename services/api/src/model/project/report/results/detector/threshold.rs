use diesel::{
    expression_methods::BoolExpressionMethods, ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl,
};

use crate::{context::DbConnection, model::project::threshold::statistic::QueryStatistic, schema};

#[derive(Debug, Clone)]
pub struct MetricsThreshold {
    pub id: i32,
    pub statistic: QueryStatistic,
}

impl MetricsThreshold {
    pub fn new(
        conn: &mut DbConnection,
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
                schema::statistic::lower_limit,
                schema::statistic::upper_limit,
            ))
            .first::<(
                i32,
                i32,
                String,
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
                    uuid,
                    test,
                    min_sample_size,
                    max_sample_size,
                    window,
                    lower_limit,
                    upper_limit,
                )| {
                    let statistic = QueryStatistic {
                        id: statistic_id,
                        uuid,
                        test,
                        min_sample_size,
                        max_sample_size,
                        window,
                        lower_limit,
                        upper_limit,
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
