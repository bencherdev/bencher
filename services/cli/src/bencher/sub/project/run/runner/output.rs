use std::fmt;

#[derive(Debug, Clone, Default)]
pub struct Output {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
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
