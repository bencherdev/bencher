mod deploy;
mod download;
mod harden;
mod install_os;
mod ssh;

use camino::Utf8PathBuf;
use clap::Parser as _;

use crate::parser::{
    TaskDeploy, TaskLogs, TaskProvision, TaskRunnerOps, TaskStart, TaskStop, TaskSub,
};
use ssh::Ssh;

#[derive(Debug)]
pub struct Task {
    sub: Sub,
}

#[derive(Debug)]
enum Sub {
    Provision(Provision),
    Deploy(Deploy),
    Start(Start),
    Stop(Stop),
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
    host: url::Url,
    runner: String,
    token: String,
    run_id: Option<u64>,
}

#[derive(Debug)]
struct Start {
    ssh: Ssh,
    host: url::Url,
    runner: String,
    token: String,
}

#[derive(Debug)]
struct Stop {
    ssh: Ssh,
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
            TaskSub::Start(start) => Self::Start(start.into()),
            TaskSub::Stop(stop) => Self::Stop(stop.into()),
            TaskSub::Logs(logs) => Self::Logs(logs.into()),
        }
    }
}

impl From<TaskProvision> for Provision {
    fn from(task: TaskProvision) -> Self {
        let TaskProvision {
            server,
            key,
            user,
            runner_binary,
        } = task;
        Self {
            ssh: Ssh::new(server, key, user),
            runner_binary,
        }
    }
}

impl From<TaskDeploy> for Deploy {
    fn from(task: TaskDeploy) -> Self {
        let TaskDeploy {
            server,
            key,
            user,
            runner,
            token,
            host,
            run_id,
        } = task;
        Self {
            ssh: Ssh::new(server, key, user),
            host,
            runner,
            token,
            run_id,
        }
    }
}

impl From<TaskStart> for Start {
    fn from(task: TaskStart) -> Self {
        let TaskStart {
            server,
            key,
            user,
            runner,
            token,
            host,
        } = task;
        Self {
            ssh: Ssh::new(server, key, user),
            host,
            runner,
            token,
        }
    }
}

impl From<TaskStop> for Stop {
    fn from(task: TaskStop) -> Self {
        let TaskStop { server, key, user } = task;
        Self {
            ssh: Ssh::new(server, key, user),
        }
    }
}

impl From<TaskLogs> for Logs {
    fn from(task: TaskLogs) -> Self {
        let TaskLogs {
            server,
            key,
            user,
            lines,
            follow,
        } = task;
        Self {
            ssh: Ssh::new(server, key, user),
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
            Self::Start(start) => start.exec(),
            Self::Stop(stop) => stop.exec(),
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
            host,
            runner,
            token,
            run_id,
        } = self;
        let (runner_binary, _temp_dir) = download::download(run_id)?;
        deploy::deploy(&ssh, Some(runner_binary.as_path()))?;
        let start = Start {
            ssh,
            host,
            runner,
            token,
        };
        start.exec()?;
        Ok(())
    }
}

impl Start {
    fn exec(self) -> anyhow::Result<()> {
        let Self {
            ssh,
            host,
            runner,
            token,
        } = self;
        println!("Configuring runner credentials...");
        ssh.run("mkdir -p /etc/systemd/system/bencher-runner.service.d")?;
        ssh.run(&format!(
            "cat > /etc/systemd/system/bencher-runner.service.d/credentials.conf << 'CRED_EOF'\n\
             [Service]\n\
             Environment=BENCHER_HOST={host}\n\
             Environment=BENCHER_RUNNER={runner}\n\
             Environment=BENCHER_RUNNER_TOKEN={token}\n\
             CRED_EOF"
        ))?;
        println!("Starting runner service...");
        ssh.run("systemctl daemon-reload")?;
        ssh.run("systemctl restart bencher-runner")?;
        ssh.run("systemctl status bencher-runner")?;
        println!("Runner is running");
        Ok(())
    }
}

impl Stop {
    fn exec(self) -> anyhow::Result<()> {
        let Self { ssh } = self;
        println!("Stopping runner service...");
        ssh.run("systemctl stop bencher-runner")?;
        println!("Runner service stopped");
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
