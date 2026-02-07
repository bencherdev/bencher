use bencher_runner::TuningConfig;
use bencher_runner::cpu::CpuLayout;
use bencher_runner::daemon::{Daemon, DaemonConfig, DaemonError};

use crate::parser::TaskDaemon;

#[derive(Debug)]
pub struct DaemonRunner {
    config: DaemonConfig,
}

impl TryFrom<TaskDaemon> for DaemonRunner {
    type Error = anyhow::Error;

    fn try_from(task: TaskDaemon) -> Result<Self, Self::Error> {
        let host =
            url::Url::parse(&task.host).map_err(|e| anyhow::anyhow!("Invalid host URL: {e}"))?;

        let tuning = if task.no_tuning {
            TuningConfig::disabled()
        } else {
            TuningConfig {
                disable_aslr: !task.aslr,
                disable_nmi_watchdog: !task.nmi_watchdog,
                swappiness: task.swappiness.or(Some(10)),
                perf_event_paranoid: task.perf_event_paranoid.or(Some(-1)),
                governor: task.governor.or_else(|| Some("performance".to_owned())),
                disable_smt: !task.smt,
                disable_turbo: !task.turbo,
            }
        };

        // Detect CPU layout for core isolation
        let cpu_layout = CpuLayout::detect();

        Ok(Self {
            config: DaemonConfig {
                host,
                token: task.token,
                runner: task.runner,
                labels: task.labels,
                poll_timeout_secs: task.poll_timeout,
                tuning,
                cpu_layout,
                max_output_size: task.max_output_size,
            },
        })
    }
}

impl DaemonRunner {
    pub fn exec(self) -> anyhow::Result<()> {
        let daemon = Daemon::new(self.config);
        match daemon.run() {
            Ok(()) => Ok(()),
            Err(DaemonError::Shutdown) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
