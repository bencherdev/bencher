use std::fmt;

use crate::RunError;

use super::{file_path::FilePath, file_size::FileSize};

#[derive(Debug, Clone, Default)]
pub struct Output {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
    pub duration: f64,
    pub result: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ExitStatus(i32);

impl From<std::process::ExitStatus> for ExitStatus {
    fn from(exit_status: std::process::ExitStatus) -> Self {
        Self(exit_status.code().unwrap_or_default())
    }
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let result = if let Some(result) = self.result.as_deref() {
            format!("\n{result}")
        } else {
            String::new()
        };
        write!(
            f,
            "{}\n{}\n{}{result}",
            self.status, self.stdout, self.stderr
        )
    }
}

impl fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Output {
    pub fn build_time(&mut self) -> Result<(), RunError> {
        // TODO turn the duration into BMF JSON
        Ok(())
    }

    pub fn file_path(&mut self, file_path: &FilePath) -> Result<(), RunError> {
        let results = file_path.get_results()?;
        self.result = Some(results);
        Ok(())
    }

    pub fn file_size(&mut self, file_size: &FileSize) -> Result<(), RunError> {
        let results = file_size.get_results()?;
        self.result = Some(results);
        Ok(())
    }

    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }

    pub fn result(self) -> String {
        self.result.unwrap_or(self.stdout)
    }
}

impl ExitStatus {
    pub fn is_success(&self) -> bool {
        self.0 == 0
    }
}
