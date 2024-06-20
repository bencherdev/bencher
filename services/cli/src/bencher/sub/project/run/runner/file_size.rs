use std::fmt;

use bencher_json::{project::measure::FILE_SIZE_SLUG, JsonNewMetric};
use camino::Utf8PathBuf;

use crate::RunError;

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

    pub fn get_results(&self) -> Result<String, RunError> {
        let mut results = Vec::with_capacity(self.0.len());
        for file_path in &self.0 {
            let file_name = file_path
                .file_name()
                .unwrap_or(file_path.as_str())
                .parse()
                .map_err(RunError::OutputFileName)?;
            let file_size_slug = FILE_SIZE_SLUG.clone().into();
            #[allow(clippy::cast_precision_loss)]
            let value = (std::fs::metadata(file_path)
                .map(|m| m.len())
                .map_err(RunError::OutputFileSize)? as f64)
                .into();
            results.push((
                file_name,
                vec![(
                    file_size_slug,
                    JsonNewMetric {
                        value,
                        ..Default::default()
                    },
                )],
            ));
        }
        let results = JsonNewMetric::results(results);
        serde_json::to_string(&results).map_err(RunError::SerializeFileSize)
    }
}
