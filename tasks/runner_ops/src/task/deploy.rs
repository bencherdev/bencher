use bencher_json::{RunnerResourceId, Secret};

use super::deploy_setup;
use super::download;
use super::merge_ssh_with_extras;
use super::ssh::Ssh;
use super::start::Start;
use crate::parser::TaskDeploy;

#[derive(Debug)]
pub struct Deploy {
    ssh: Ssh,
    host: url::Url,
    runner: RunnerResourceId,
    key: Secret,
    run_id: Option<u64>,
}

impl TryFrom<TaskDeploy> for Deploy {
    type Error = anyhow::Error;

    fn try_from(task: TaskDeploy) -> anyhow::Result<Self> {
        let TaskDeploy {
            runner,
            server,
            ssh,
            user,
            key,
            host,
            run_id,
        } = task;
        let (ssh, host, runner, key) = merge_ssh_with_extras(runner, server, ssh, user, key, host)?;
        Ok(Self {
            ssh,
            host,
            runner,
            key,
            run_id,
        })
    }
}

impl Deploy {
    pub fn exec(self) -> anyhow::Result<()> {
        let Self {
            ssh,
            host,
            runner,
            key,
            run_id,
        } = self;
        let (runner_binary, _temp_dir) = download::download(run_id)?;
        deploy_setup::deploy(&ssh, Some(runner_binary.as_path()))?;
        let start = Start::new(ssh, host, runner, key, false);
        start.exec()?;
        Ok(())
    }
}
