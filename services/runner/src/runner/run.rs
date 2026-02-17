use bencher_runner::{PerfEventParanoid, RunArgs, Swappiness, TuningConfig};

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
                    Some(task.output)
                },
                max_output_size: task.max_output_size,
                max_file_count: task.max_file_count,
                network: task.network,
                tuning,
                grace_period: task.grace_period,
                firecracker_log_level: task.firecracker_log_level,
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
