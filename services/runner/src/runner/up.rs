use bencher_runner::up::{Up as RunnerUp, UpConfig, UpError};

use crate::error::RunnerCliError;
use crate::parser::CliUp;

#[derive(Debug)]
pub struct Up {
    config: UpConfig,
}

impl TryFrom<CliUp> for Up {
    type Error = RunnerCliError;

    fn try_from(task: CliUp) -> Result<Self, Self::Error> {
        let tuning = task.tuning.try_into()?;

        Ok(Self {
            config: UpConfig {
                host: task.host,
                key: task.key,
                runner: task.runner,
                poll_timeout_secs: task.poll_timeout,
                tuning,
                cpu_layout: None,
                max_output_size: task.max_output_size,
                max_file_count: task.max_file_count,
                max_symlinks: task.max_symlinks,
                grace_period: task.grace_period,
                sandbox_log_level: task.sandbox_log_level,
                allow_no_sandbox: task.danger_allow_no_sandbox,
                no_auto_update: task.no_auto_update,
            },
        })
    }
}

impl Up {
    pub fn exec(self) -> Result<(), RunnerCliError> {
        let up = RunnerUp::new(self.config);
        match up.run() {
            Ok(()) | Err(UpError::Shutdown) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
