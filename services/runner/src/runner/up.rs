use bencher_runner::cpu::CpuLayout;
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

        // Detect CPU layout for core isolation
        let cpu_layout = CpuLayout::detect();

        Ok(Self {
            config: UpConfig {
                host: task.host,
                token: task.token,
                runner: task.runner,
                poll_timeout_secs: task.poll_timeout,
                tuning,
                cpu_layout,
                max_output_size: task.max_output_size,
                max_file_count: task.max_file_count,
                grace_period: task.grace_period,
                firecracker_log_level: task.firecracker_log_level,
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
