use bencher_runner::cpu::CpuLayout;
use bencher_runner::daemon::{Daemon, DaemonConfig, DaemonError};

use crate::error::RunnerCliError;
use crate::parser::CliUp;

#[derive(Debug)]
pub struct Up {
    config: DaemonConfig,
}

impl TryFrom<CliUp> for Up {
    type Error = RunnerCliError;

    fn try_from(task: CliUp) -> Result<Self, Self::Error> {
        let tuning = task.tuning.try_into()?;

        // Detect CPU layout for core isolation
        let cpu_layout = CpuLayout::detect();

        Ok(Self {
            config: DaemonConfig {
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
        let daemon = Daemon::new(self.config);
        match daemon.run() {
            Ok(()) | Err(DaemonError::Shutdown) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
