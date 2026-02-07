use bencher_runner::{RunArgs, TuningConfig};

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
                swappiness: task.swappiness.or(Some(10)),
                perf_event_paranoid: task.perf_event_paranoid.or(Some(-1)),
                governor: task.governor.or_else(|| Some("performance".to_owned())),
                disable_smt: !task.smt,
                disable_turbo: !task.turbo,
            }
        };

        Ok(Self {
            args: RunArgs {
                image: task.image,
                token: task.token,
                vcpus: task.vcpus,
                memory_mib: task.memory,
                timeout_secs: task.timeout,
                output_file: task.output,
                max_output_size: task.max_output_size,
                tuning,
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
