use bencher_runner::RunArgs;

use crate::parser::TaskRun;

#[derive(Debug)]
pub struct Run {
    args: RunArgs,
}

impl TryFrom<TaskRun> for Run {
    type Error = anyhow::Error;

    fn try_from(task: TaskRun) -> Result<Self, Self::Error> {
        Ok(Self {
            args: RunArgs {
                image: task.image,
                token: task.token,
                vcpus: task.vcpus,
                memory_mib: task.memory,
                timeout_secs: task.timeout,
                output_file: task.output,
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
