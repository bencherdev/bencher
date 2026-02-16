use bencher_runner::cpu::CpuLayout;
use bencher_runner::daemon::{Daemon, DaemonConfig, DaemonError};
use bencher_runner::{FirecrackerLogLevel, PerfEventParanoid, Swappiness, TuningConfig};

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
                swappiness: task
                    .swappiness
                    .map(Swappiness::try_from)
                    .transpose()?
                    .or(Some(Swappiness::DEFAULT)),
                perf_event_paranoid: task
                    .perf_event_paranoid
                    .map(PerfEventParanoid::try_from)
                    .transpose()?
                    .or(Some(PerfEventParanoid::DEFAULT)),
                governor: task.governor.or_else(|| Some("performance".to_owned())),
                disable_smt: !task.smt,
                disable_turbo: !task.turbo,
            }
        };

        // Detect CPU layout for core isolation
        let cpu_layout = CpuLayout::detect();

        let firecracker_log_level: FirecrackerLogLevel = task
            .firecracker_log_level
            .parse()
            .map_err(|e: String| anyhow::anyhow!(e))?;

        Ok(Self {
            config: DaemonConfig {
                host,
                token: task.token,
                runner: task.runner,
                poll_timeout_secs: task.poll_timeout,
                tuning,
                cpu_layout,
                max_output_size: task.max_output_size,
                firecracker_log_level,
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
