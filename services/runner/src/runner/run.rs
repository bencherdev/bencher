use bencher_runner::{FirecrackerLogLevel, PerfEventParanoid, RunArgs, Swappiness, TuningConfig};
use camino::Utf8PathBuf;

use crate::parser::TaskRun;

#[derive(Debug)]
pub struct Run {
    args: RunArgs,
}

impl TryFrom<TaskRun> for Run {
    type Error = anyhow::Error;

    fn try_from(task: TaskRun) -> Result<Self, Self::Error> {
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

        let vcpus = task.vcpus.map(bencher_runner::Cpu::try_from).transpose()?;
        let memory = task.memory.map(bencher_runner::Memory::from_mib);
        let disk = task.disk.map(bencher_runner::Disk::from_mib);

        let firecracker_log_level: FirecrackerLogLevel = task
            .firecracker_log_level
            .parse()
            .map_err(|e: String| anyhow::anyhow!(e))?;

        Ok(Self {
            args: RunArgs {
                image: task.image,
                token: task.token,
                vcpus,
                memory,
                disk,
                timeout_secs: task.timeout,
                file_paths: if task.output.is_empty() {
                    None
                } else {
                    Some(task.output.into_iter().map(Utf8PathBuf::from).collect())
                },
                max_output_size: task.max_output_size,
                network: task.network,
                tuning,
                firecracker_log_level,
            },
        })
    }
}

impl Run {
    pub fn exec(self) -> anyhow::Result<()> {
        bencher_runner::run_with_args(&self.args)?;
        Ok(())
    }
}
