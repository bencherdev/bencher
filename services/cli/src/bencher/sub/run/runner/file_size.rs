use std::fmt;

use bencher_json::{
    BmfVersion, JsonNewMetric,
    project::{
        measure::built_in::{self, BuiltInMeasure as _},
        metric::{JsonNewResults, MetricResults},
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

    pub fn paths(&self) -> &[Utf8PathBuf] {
        &self.0
    }

    pub fn get_results(
        &self,
        build_command: Option<&BuildCommand>,
        bmf_version: BmfVersion,
    ) -> Result<String, RunError> {
        let mut metric_results = self.to_metric_results()?;
        if let Some(build_command) = build_command {
            metric_results.extend(build_command.to_metric_results()?);
        }
        let results = JsonNewResults::new(metric_results, bmf_version);
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
            #[expect(
                clippy::cast_precision_loss,
                reason = "file size u64 to f64 is acceptable precision"
            )]
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

#[cfg(test)]
mod tests {
    use bencher_json::BmfVersion;
    use camino::Utf8PathBuf;

    use super::super::{build_time::BuildTime, command::Command};
    use super::FileSize;

    // Kills: file sizes, or the build time beside them, written in one shape whatever the
    // report's version.
    #[test]
    fn file_size_is_written_at_the_report_s_version() {
        let path = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        #[expect(
            clippy::cast_precision_loss,
            reason = "a manifest's size is exact as f64"
        )]
        let size = std::fs::metadata(&path).unwrap().len() as f64;
        let build_command =
            BuildTime.command(&Command::new_exec("./build.sh".to_owned(), Vec::new()), 1.5);
        for (bmf_version, results) in [
            (
                BmfVersion::V0,
                serde_json::json!({
                    "Cargo.toml": {
                        "file-size": { "value": size, "lower_value": null, "upper_value": null }
                    },
                    "./build.sh": {
                        "build-time": { "value": 1.5, "lower_value": null, "upper_value": null }
                    },
                }),
            ),
            (
                BmfVersion::V1,
                serde_json::json!({
                    "Cargo.toml": [{ "measures": { "file-size": { "value": size } } }],
                    "./build.sh": [{ "measures": { "build-time": { "value": 1.5 } } }],
                }),
            ),
        ] {
            let written = FileSize::new(vec![path.clone()])
                .get_results(Some(&build_command), bmf_version)
                .unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&written).unwrap(),
                results,
                "{bmf_version}"
            );
        }
    }
}
