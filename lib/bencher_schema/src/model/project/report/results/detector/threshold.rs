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

/// One threshold that may check a metric row: what it checks, and the model it runs.
#[derive(Debug, Clone)]
pub struct Threshold {
    pub id: ThresholdId,
    /// The UUID, which is what orders the candidates.
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
    /// Every threshold of one (branch, testbed) that has a model, grouped by measure.
    ///
    /// A report reads this once and matches every metric row it ingests against it in
    /// memory, so however many thresholds check a series, ingest asks the threshold
    /// table one question per report.
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
        // UUID order, which is the order the boundaries a metric row earns are written
        // in and read back in.
        for candidates in by_measure.values_mut() {
            candidates.sort_by_key(|candidate| candidate.uuid);
        }
        Ok(by_measure)
    }

    /// Whether this threshold checks a variant.
    ///
    /// The name is matched by the caller, which reads it straight off `metric`.
    pub fn checks(&self, variant: &ParameterSet) -> bool {
        self.parameters
            .as_ref()
            .is_none_or(|parameters| parameters.matches(variant))
    }
}
