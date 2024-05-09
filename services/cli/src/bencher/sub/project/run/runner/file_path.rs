use std::fmt;

use camino::Utf8PathBuf;

use crate::RunError;

#[derive(Debug, Clone)]
pub struct FilePath(Utf8PathBuf);

impl fmt::Display for FilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FilePath {
    pub fn new(file_path: Utf8PathBuf) -> Self {
        Self(file_path)
    }

    pub fn get_results(&self) -> Result<String, RunError> {
        std::fs::read_to_string(&self.0).map_err(RunError::OutputFileRead)
    }
}
