use bencher_json::{
    JsonNewMetric,
    project::{
        measure::built_in::{self, BuiltInMeasure as _},
        metric::MetricResults,
    },
};

use crate::RunError;

#[derive(Debug, Clone, Copy)]
pub struct BuildTime;

impl BuildTime {
    #[allow(clippy::unused_self)]
    pub fn command(self, name: String, duration: f64) -> BuildCommand {
        BuildCommand { name, duration }
    }
}

#[derive(Debug, Clone)]
pub struct BuildCommand {
    name: String,
    duration: f64,
}

impl BuildCommand {
    pub fn get_results(self) -> Result<String, RunError> {
        let results = JsonNewMetric::results(self.to_metric_results()?);
        serde_json::to_string(&results).map_err(RunError::SerializeBuildTime)
    }

    pub fn to_metric_results(&self) -> Result<MetricResults, RunError> {
        Ok(vec![(
            self.name.parse().map_err(RunError::CommandName)?,
            vec![(
                built_in::json::BuildTime::name_id(),
                JsonNewMetric {
                    value: self.duration.into(),
                    ..Default::default()
                },
            )],
        )])
    }
}
