use std::collections::HashMap;

use bencher_json::{
    Boundary, MetricName, ModelTest, ParameterFilter, ParameterSet, SampleSize, ThresholdUuid,
    Window,
};
use diesel::{
    ExpressionMethods as _, JoinOnDsl as _, NullableExpressionMethods as _, QueryDsl as _,
    RunQueryDsl as _, SelectableHelper as _,
};

use crate::{
    context::DbConnection,
    model::project::{
        branch::BranchId,
        measure::MeasureId,
        testbed::TestbedId,
        threshold::{
            ThresholdId,
            model::{ModelId, QueryModel},
        },
    },
    schema,
};

#[derive(Debug, Clone)]
pub struct Threshold {
    pub id: ThresholdId,
    pub uuid: ThresholdUuid,
    /// The variants this threshold checks. `None` checks every variant.
    pub parameters: Option<ParameterFilter>,
    /// The name this threshold checks. A threshold that names none checks the
    /// conventional `value` name, and a threshold always checks exactly one name.
    pub metric: MetricName,
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
    pub fn load(
        conn: &mut DbConnection,
        branch_id: BranchId,
        testbed_id: TestbedId,
    ) -> diesel::QueryResult<HashMap<MeasureId, Vec<Self>>> {
        let thresholds = schema::model::table
            .inner_join(
                schema::threshold::table
                    .on(schema::model::id.nullable().eq(schema::threshold::model_id)),
            )
            .filter(schema::threshold::branch_id.eq(branch_id))
            .filter(schema::threshold::testbed_id.eq(testbed_id))
            .select((
                schema::threshold::id,
                schema::threshold::uuid,
                schema::threshold::parameters,
                schema::threshold::measure_id,
                schema::threshold::metric,
                QueryModel::as_select(),
            ))
            .load::<(
                ThresholdId,
                ThresholdUuid,
                Option<ParameterFilter>,
                MeasureId,
                Option<MetricName>,
                QueryModel,
            )>(conn)?;

        let mut by_measure: HashMap<MeasureId, Vec<Self>> = HashMap::new();
        for (id, uuid, parameters, measure_id, metric, query_model) in thresholds {
            let QueryModel {
                id: model_id,
                test,
                min_sample_size,
                max_sample_size,
                window,
                lower_boundary,
                upper_boundary,
                ..
            } = query_model;
            by_measure.entry(measure_id).or_default().push(Self {
                id,
                uuid,
                parameters,
                metric: metric.unwrap_or_else(MetricName::value),
                model: ThresholdModel {
                    id: model_id,
                    test,
                    min_sample_size,
                    max_sample_size,
                    window,
                    lower_boundary,
                    upper_boundary,
                },
            });
        }
        for candidates in by_measure.values_mut() {
            candidates.sort_by_key(|candidate| candidate.uuid);
        }
        Ok(by_measure)
    }

    pub fn checks(&self, variant: &ParameterSet) -> bool {
        self.parameters
            .as_ref()
            .is_none_or(|parameters| parameters.matches(variant))
    }
}
