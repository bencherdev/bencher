use std::fmt;

use bencher_json::{
    JsonNewMetric,
    project::{
        measure::built_in::{self, BuiltInMeasure as _},
        metric::MetricResults,
    },
};
use camino::Utf8PathBuf;

use crate::RunError;

use super::build_time::BuildCommand;

#[derive(Debug, Clone)]
pub struct FileSize(Vec<Utf8PathBuf>);

impl fmt::Display for FileSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.0
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<&str>>()
                .join(", ")
        )
    }
}

impl FileSize {
    pub fn new(file_paths: Vec<Utf8PathBuf>) -> Self {
        Self(file_paths)
    }

    pub fn get_results(&self, build_command: Option<&BuildCommand>) -> Result<String, RunError> {
        let mut metric_results = self.to_metric_results()?;
        if let Some(build_command) = build_command {
            metric_results.extend(build_command.to_metric_results()?);
        }
        let results = JsonNewMetric::results(metric_results);
        serde_json::to_string(&results).map_err(RunError::SerializeFileSize)
    }

    fn to_metric_results(&self) -> Result<MetricResults, RunError> {
        let mut metric_results = Vec::with_capacity(self.0.len());
        for file_path in &self.0 {
            let file_name = file_path
                .file_name()
                .unwrap_or(file_path.as_str())
                .parse()
                .map_err(RunError::OutputFileName)?;
            #[allow(clippy::cast_precision_loss)]
            let value = (std::fs::metadata(file_path)
                .map(|m| m.len())
                .map_err(RunError::OutputFileSize)? as f64)
                .into();
            metric_results.push((
                file_name,
                vec![(
                    built_in::json::FileSize::name_id(),
                    JsonNewMetric {
                        value,
                        ..Default::default()
                    },
                )],
            ));
        }
        Ok(metric_results)
    }
}
