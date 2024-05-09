use std::fmt;

use bencher_json::{project::measure::FILE_SIZE_SLUG, JsonMetric};
use camino::Utf8PathBuf;

use crate::RunError;

#[derive(Debug, Clone)]
pub struct FileSize(Utf8PathBuf);

impl fmt::Display for FileSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FileSize {
    pub fn new(file_path: Utf8PathBuf) -> Self {
        Self(file_path)
    }

    pub fn get_results(&self) -> Result<String, RunError> {
        let file_name = self
            .0
            .file_name()
            .unwrap_or(self.0.as_str())
            .parse()
            .map_err(RunError::OutputFileName)?;
        let file_size_slug = FILE_SIZE_SLUG.clone().into();
        #[allow(clippy::cast_precision_loss)]
        let value = (std::fs::metadata(&self.0)
            .map(|m| m.len())
            .map_err(RunError::OutputFileSize)? as f64)
            .into();
        let results = JsonMetric::new_results(vec![(
            file_name,
            vec![(
                file_size_slug,
                JsonMetric {
                    value,
                    ..Default::default()
                },
            )],
        )]);
        serde_json::to_string(&results).map_err(RunError::SerializeFileSize)
    }
}
