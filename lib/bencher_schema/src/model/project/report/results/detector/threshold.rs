use std::collections::HashMap;

use bencher_json::{
    Boundary, DateTime, MetricName, ModelTest, ParameterFilter, ParameterSet, SampleSize, Window,
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

/// One threshold that may gate a metric row: what it gates, and the model it runs.
#[derive(Debug, Clone)]
pub struct Threshold {
    pub id: ThresholdId,
    /// The name this threshold gates. A threshold that names none gates the
    /// conventional `value` name, and a threshold always gates exactly one name.
    pub metric: MetricName,
    /// The grid points this threshold gates. `None` gates every grid point.
    pub parameters: Option<ParameterFilter>,
    /// When the threshold was created, which is what orders the candidates.
    pub created: DateTime,
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
    /// memory, so however many thresholds gate a series, ingest asks the threshold
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
                schema::threshold::measure_id,
                schema::threshold::metric,
                schema::threshold::parameters,
                schema::threshold::created,
                QueryModel::as_select(),
            ))
            .load::<(
                ThresholdId,
                MeasureId,
                Option<MetricName>,
                Option<ParameterFilter>,
                DateTime,
                QueryModel,
            )>(conn)?;

        let mut by_measure: HashMap<MeasureId, Vec<Self>> = HashMap::new();
        for (id, measure_id, metric, parameters, created, query_model) in thresholds {
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
                metric: metric.unwrap_or_else(MetricName::value),
                parameters,
                created,
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
        // Creation order, oldest first, which is the order the boundaries one metric
        // row earns are written in and the order they are read back in. Sorted here
        // rather than in SQL: a measure's candidates are a handful of rows, and an
        // `ORDER BY` would put a sort behind an index lookup that has none.
        for candidates in by_measure.values_mut() {
            candidates.sort_by_key(|candidate| (candidate.created.timestamp(), candidate.id));
        }
        Ok(by_measure)
    }

    /// Whether this threshold gates a grid point.
    ///
    /// The name is matched by the caller, which reads it straight off `metric`.
    pub fn gates(&self, grid_point: &ParameterSet) -> bool {
        self.parameters
            .as_ref()
            .is_none_or(|parameters| parameters.matches(grid_point))
    }
}
