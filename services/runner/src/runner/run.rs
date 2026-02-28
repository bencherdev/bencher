use bencher_runner::RunArgs;

use crate::error::RunnerCliError;
use crate::parser::CliRun;

#[derive(Debug)]
pub struct Run {
    args: RunArgs,
}

impl TryFrom<CliRun> for Run {
    type Error = RunnerCliError;

    fn try_from(task: CliRun) -> Result<Self, Self::Error> {
        let tuning = task.tuning.try_into()?;

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

        let env = task.env.map(bencher_parser::parse_env);

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
                iter: task.iter,
                allow_failure: task.allow_failure,
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
