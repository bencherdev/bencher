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

impl From<TaskRunnerOps> for Task {
    fn from(task: TaskRunnerOps) -> Self {
        Self {
            sub: task.sub.into(),
        }
    }
}

impl From<TaskSub> for Sub {
    fn from(sub: TaskSub) -> Self {
        match sub {
            TaskSub::Provision(provision) => Self::Provision(provision.into()),
            TaskSub::Deploy(deploy) => Self::Deploy(deploy.into()),
            TaskSub::Logs(logs) => Self::Logs(logs.into()),
        }
    }
}

impl From<TaskProvision> for Provision {
    fn from(task: TaskProvision) -> Self {
        let TaskProvision {
            host,
            key,
            user,
            runner_binary,
        } = task;
        Self {
            ssh: Ssh::new(host, key, user),
            runner_binary,
        }
    }
}

impl From<TaskDeploy> for Deploy {
    fn from(task: TaskDeploy) -> Self {
        let TaskDeploy {
            host,
            key,
            user,
            runner,
            token,
            run_id,
        } = task;
        Self {
            ssh: Ssh::new(host, key, user),
            runner,
            token,
            run_id,
        }
    }
}

impl From<TaskLogs> for Logs {
    fn from(task: TaskLogs) -> Self {
        let TaskLogs {
            host,
            key,
            user,
            lines,
            follow,
        } = task;
        Self {
            ssh: Ssh::new(host, key, user),
            lines,
            follow,
        }
    }
}

impl Task {
    pub fn new() -> Self {
        TaskRunnerOps::parse().into()
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
        let (runner_binary, _temp_dir) = download::download(run_id)?;
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
