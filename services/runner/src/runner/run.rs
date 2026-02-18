use std::collections::HashMap;

use bencher_runner::{PerfEventParanoid, RunArgs, Swappiness, TuningConfig};

use crate::error::RunnerCliError;
use crate::parser::CliRun;

#[derive(Debug)]
pub struct Run {
    args: RunArgs,
}

impl TryFrom<CliRun> for Run {
    type Error = RunnerCliError;

    fn try_from(task: CliRun) -> Result<Self, Self::Error> {
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
        let memory = task
            .memory
            .map(|mib| {
                bencher_runner::Memory::from_mib(mib).ok_or(RunnerCliError::InvalidMemory(mib))
            })
            .transpose()?;
        let disk = task
            .disk
            .map(|mib| bencher_runner::Disk::from_mib(mib).ok_or(RunnerCliError::InvalidDisk(mib)))
            .transpose()?;

        // Convert --env KEY=VALUE strings into a HashMap
        let env = task.env.map(|env_args| {
            env_args
                .into_iter()
                .filter_map(|s| s.split_once('=').map(|(k, v)| (k.to_owned(), v.to_owned())))
                .collect::<HashMap<String, String>>()
        });

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
                entrypoint: task.entrypoint,
                cmd: task.cmd,
                env,
                network: task.network,
                tuning,
                grace_period: task.grace_period,
                firecracker_log_level: task.firecracker_log_level,
            },
        })
    }
}

impl Run {
    pub fn exec(self) -> Result<(), RunnerCliError> {
        bencher_runner::run_with_args(&self.args)?;
        Ok(())
    }
}
