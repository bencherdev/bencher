use diesel::{
    expression_methods::BoolExpressionMethods,
    JoinOnDsl,
    QueryDsl,
    RunQueryDsl,
    SqliteConnection,
};
use dropshot::HttpError;

use crate::{
    db::{
        model::threshold::{
            statistic::QueryStatistic,
            PerfKind,
        },
        schema,
    },
    diesel::ExpressionMethods,
};

pub struct Threshold {
    pub id:        i32,
    pub statistic: QueryStatistic,
}

impl Threshold {
    pub fn new(
        conn: &mut SqliteConnection,
        branch_id: i32,
        testbed_id: i32,
        kind: PerfKind,
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
                    .and(schema::threshold::kind.eq(kind as i32)),
            )
            .select((
                schema::threshold::id,
                schema::statistic::id,
                schema::statistic::uuid,
                schema::statistic::test,
                schema::statistic::sample_size,
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
                Option<f32>,
                Option<f32>,
            )>(conn)
            .map(
                |(
                    threshold_id,
                    statistic_id,
                    uuid,
                    test,
                    sample_size,
                    window,
                    left_side,
                    right_side,
                )|
                 -> Result<Self, HttpError> {
                    let statistic = QueryStatistic {
                        id: statistic_id,
                        uuid,
                        test,
                        sample_size,
                        window,
                        left_side,
                        right_side,
                    };
                    Ok(Self {
                        id: threshold_id,
                        statistic,
                    })
                },
            )
            .ok()?
            .ok()
    }
}
