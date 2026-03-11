mod deploy;
mod download;
mod harden;
mod install_os;
mod ssh;

use camino::Utf8PathBuf;
use clap::Parser as _;

use crate::parser::{TaskDeploy, TaskLogs, TaskProvision, TaskRunnerOps, TaskSub};
use ssh::Ssh;

#[derive(Debug)]
pub struct Task {
    sub: Sub,
}

#[derive(Debug)]
enum Sub {
    Provision(Provision),
    Deploy(Deploy),
    Logs(Logs),
}

#[derive(Debug)]
struct Provision {
    ssh: Ssh,
    runner_binary: Option<Utf8PathBuf>,
}

#[derive(Debug)]
struct Deploy {
    ssh: Ssh,
    runner: String,
    token: String,
    run_id: Option<u64>,
}

#[derive(Debug)]
struct Logs {
    ssh: Ssh,
    lines: Option<u32>,
    follow: bool,
}

impl TryFrom<TaskRunnerOps> for Task {
    type Error = anyhow::Error;

    fn try_from(task: TaskRunnerOps) -> Result<Self, Self::Error> {
        Ok(Self {
            sub: task.sub.try_into()?,
        })
    }
}

impl TryFrom<TaskSub> for Sub {
    type Error = anyhow::Error;

    fn try_from(sub: TaskSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            TaskSub::Provision(provision) => Self::Provision(provision.try_into()?),
            TaskSub::Deploy(deploy) => Self::Deploy(deploy.try_into()?),
            TaskSub::Logs(logs) => Self::Logs(logs.try_into()?),
        })
    }
}

impl TryFrom<TaskProvision> for Provision {
    type Error = anyhow::Error;

    fn try_from(task: TaskProvision) -> Result<Self, Self::Error> {
        let TaskProvision {
            host,
            key,
            user,
            runner_binary,
        } = task;
        Ok(Self {
            ssh: Ssh::new(host, key, user),
            runner_binary,
        })
    }
}

impl TryFrom<TaskDeploy> for Deploy {
    type Error = anyhow::Error;

    fn try_from(task: TaskDeploy) -> Result<Self, Self::Error> {
        let TaskDeploy {
            host,
            key,
            user,
            runner,
            token,
            run_id,
        } = task;
        Ok(Self {
            ssh: Ssh::new(host, key, user),
            runner,
            token,
            run_id,
        })
    }
}

impl TryFrom<TaskLogs> for Logs {
    type Error = anyhow::Error;

    fn try_from(task: TaskLogs) -> Result<Self, Self::Error> {
        let TaskLogs {
            host,
            key,
            user,
            lines,
            follow,
        } = task;
        Ok(Self {
            ssh: Ssh::new(host, key, user),
            lines,
            follow,
        })
    }
}

impl Task {
    pub fn new() -> anyhow::Result<Self> {
        TaskRunnerOps::parse().try_into()
    }

    pub fn exec(self) -> anyhow::Result<()> {
        self.sub.exec()
    }
}

impl Sub {
    fn exec(self) -> anyhow::Result<()> {
        match self {
            Self::Provision(provision) => provision.exec(),
            Self::Deploy(deploy) => deploy.exec(),
            Self::Logs(logs) => logs.exec(),
        }
    }
}

impl Provision {
    fn exec(self) -> anyhow::Result<()> {
        let Self { ssh, runner_binary } = self;
        install_os::install_os(&ssh)?;
        harden::harden(&ssh)?;
        deploy::deploy(&ssh, runner_binary.as_deref())?;
        Ok(())
    }
}

impl Deploy {
    fn exec(self) -> anyhow::Result<()> {
        let Self {
            ssh,
            runner,
            token,
            run_id,
        } = self;
        let runner_binary = download::download(run_id)?;
        deploy::deploy(&ssh, Some(runner_binary.as_path()))?;
        deploy::start(&ssh, &runner, &token)?;
        Ok(())
    }
}

impl Logs {
    fn exec(self) -> anyhow::Result<()> {
        use std::fmt::Write as _;

        let Self { ssh, lines, follow } = self;
        let mut cmd = String::from("journalctl -u bencher-runner --no-pager");
        if let Some(n) = lines {
            let _ = write!(cmd, " -n {n}");
        }
        if follow {
            cmd.push_str(" -f");
        }
        ssh.exec(&cmd)
    }
}
