use bencher_json::{Boundary, ModelTest, SampleSize, Window};
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
            model::{ModelId, QueryModel},
            ThresholdId,
        },
    },
    schema,
};

#[derive(Debug, Clone)]
pub struct Threshold {
    pub id: ThresholdId,
    pub model: ThresholdModel,
}

#[derive(Debug, Clone)]
pub struct ThresholdModel {
    pub id: ModelId,
    pub test: ModelTest,
    pub min_sample_size: Option<SampleSize>,
    pub max_sample_size: Option<SampleSize>,
    pub window: Option<Window>,
    pub lower_boundary: Option<Boundary>,
    pub upper_boundary: Option<Boundary>,
}

impl Threshold {
    pub fn new(
        conn: &mut DbConnection,
        branch_id: BranchId,
        testbed_id: TestbedId,
        measure_id: MeasureId,
    ) -> Option<Self> {
        schema::model::table
            .inner_join(
                schema::threshold::table
                    .on(schema::model::id.nullable().eq(schema::threshold::model_id)),
            )
            .filter(schema::threshold::branch_id.eq(branch_id))
            .filter(schema::threshold::testbed_id.eq(testbed_id))
            .filter(schema::threshold::measure_id.eq(measure_id))
            .select((schema::threshold::id, QueryModel::as_select()))
            .first::<(ThresholdId, QueryModel)>(conn)
            .map(|(threshold_id, query_model)| {
                let QueryModel {
                    id,
                    test,
                    min_sample_size,
                    max_sample_size,
                    window,
                    lower_boundary,
                    upper_boundary,
                    ..
                } = query_model;
                let model = ThresholdModel {
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
                    model,
                }
            })
            .ok()
    }
}
