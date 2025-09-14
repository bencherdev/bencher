use std::fmt;

use camino::Utf8PathBuf;

use crate::RunError;

#[derive(Debug, Clone)]
pub struct FilePath(Vec<Utf8PathBuf>);

impl fmt::Display for FilePath {
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

impl FilePath {
    pub fn new(file_paths: Vec<Utf8PathBuf>) -> Self {
        Self(file_paths)
    }

    pub fn get_results(&self) -> Result<Vec<String>, RunError> {
        let mut results = Vec::new();
        for path in &self.0 {
            let result = std::fs::read_to_string(path).map_err(RunError::OutputFileRead)?;
            results.push(result);
        }
        Ok(results)
    }
}
