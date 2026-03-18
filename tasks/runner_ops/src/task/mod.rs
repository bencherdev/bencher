mod deploy;
mod download;
mod harden;
mod install_os;
mod ssh;

use camino::Utf8PathBuf;
use clap::Parser as _;

use crate::parser::server::load_server;
use crate::parser::{
    TaskDeploy, TaskLogs, TaskProvision, TaskRunnerOps, TaskStart, TaskStop, TaskSub,
};
use ssh::Ssh;

const DEFAULT_USER: &str = "root";
#[expect(clippy::expect_used, reason = "known-valid constant URL")]
static DEFAULT_HOST: std::sync::LazyLock<url::Url> =
    std::sync::LazyLock::new(|| "https://api.bencher.dev".parse().expect("valid URL"));

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

impl TryFrom<TaskRunnerOps> for Task {
    type Error = anyhow::Error;

    fn try_from(task: TaskRunnerOps) -> anyhow::Result<Self> {
        Ok(Self {
            sub: task.sub.try_into()?,
        })
    }
}

impl TryFrom<TaskSub> for Sub {
    type Error = anyhow::Error;

    fn try_from(sub: TaskSub) -> anyhow::Result<Self> {
        Ok(match sub {
            TaskSub::Provision(provision) => Self::Provision(provision.try_into()?),
            TaskSub::Deploy(deploy) => Self::Deploy(deploy.try_into()?),
            TaskSub::Start(start) => Self::Start(start.try_into()?),
            TaskSub::Stop(stop) => Self::Stop(stop.try_into()?),
            TaskSub::Logs(logs) => Self::Logs(logs.try_into()?),
        })
    }
}

impl TryFrom<TaskProvision> for Provision {
    type Error = anyhow::Error;

    fn try_from(task: TaskProvision) -> anyhow::Result<Self> {
        let TaskProvision {
            name,
            server,
            key,
            user,
            runner_binary,
        } = task;
        let (server, key, user) = merge_ssh(name.as_deref(), server, key, user)?;
        Ok(Self {
            ssh: Ssh::new(server, key, user),
            runner_binary,
        })
    }
}

impl TryFrom<TaskDeploy> for Deploy {
    type Error = anyhow::Error;

    fn try_from(task: TaskDeploy) -> anyhow::Result<Self> {
        let TaskDeploy {
            name,
            server,
            key,
            user,
            runner,
            token,
            host,
            run_id,
        } = task;
        let file = name.as_deref().map(load_server).transpose()?;
        let server = server
            .or(file.as_ref().map(|f| f.server.clone()))
            .ok_or_else(|| anyhow::anyhow!("--server is required"))?;
        let key = key
            .or(file.as_ref().and_then(|f| f.key.clone()))
            .ok_or_else(|| anyhow::anyhow!("--key is required"))?;
        let user = user
            .or(file.as_ref().and_then(|f| f.user.clone()))
            .unwrap_or_else(|| DEFAULT_USER.into());
        let runner = runner
            .or(file.as_ref().and_then(|f| f.runner.clone()))
            .ok_or_else(|| anyhow::anyhow!("--runner is required"))?;
        let token = token
            .or(file.as_ref().and_then(|f| f.token.clone()))
            .ok_or_else(|| anyhow::anyhow!("--token is required"))?;
        let host = host
            .or(file.as_ref().and_then(|f| f.host.clone()))
            .unwrap_or_else(|| DEFAULT_HOST.clone());
        Ok(Self {
            ssh: Ssh::new(server, key, user),
            host,
            runner,
            token,
            run_id,
        })
    }
}

impl TryFrom<TaskStart> for Start {
    type Error = anyhow::Error;

    fn try_from(task: TaskStart) -> anyhow::Result<Self> {
        let TaskStart {
            name,
            server,
            key,
            user,
            runner,
            token,
            host,
        } = task;
        let file = name.as_deref().map(load_server).transpose()?;
        let server = server
            .or(file.as_ref().map(|f| f.server.clone()))
            .ok_or_else(|| anyhow::anyhow!("--server is required"))?;
        let key = key
            .or(file.as_ref().and_then(|f| f.key.clone()))
            .ok_or_else(|| anyhow::anyhow!("--key is required"))?;
        let user = user
            .or(file.as_ref().and_then(|f| f.user.clone()))
            .unwrap_or_else(|| DEFAULT_USER.into());
        let runner = runner
            .or(file.as_ref().and_then(|f| f.runner.clone()))
            .ok_or_else(|| anyhow::anyhow!("--runner is required"))?;
        let token = token
            .or(file.as_ref().and_then(|f| f.token.clone()))
            .ok_or_else(|| anyhow::anyhow!("--token is required"))?;
        let host = host
            .or(file.as_ref().and_then(|f| f.host.clone()))
            .unwrap_or_else(|| DEFAULT_HOST.clone());
        Ok(Self {
            ssh: Ssh::new(server, key, user),
            host,
            runner,
            token,
        })
    }
}

impl TryFrom<TaskStop> for Stop {
    type Error = anyhow::Error;

    fn try_from(task: TaskStop) -> anyhow::Result<Self> {
        let TaskStop {
            name,
            server,
            key,
            user,
        } = task;
        let (server, key, user) = merge_ssh(name.as_deref(), server, key, user)?;
        Ok(Self {
            ssh: Ssh::new(server, key, user),
        })
    }
}

impl TryFrom<TaskLogs> for Logs {
    type Error = anyhow::Error;

    fn try_from(task: TaskLogs) -> anyhow::Result<Self> {
        let TaskLogs {
            name,
            server,
            key,
            user,
            lines,
            follow,
        } = task;
        let (server, key, user) = merge_ssh(name.as_deref(), server, key, user)?;
        Ok(Self {
            ssh: Ssh::new(server, key, user),
            lines,
            follow,
        })
    }
}

/// Merge SSH fields from CLI flags and optional server config file.
fn merge_ssh(
    name: Option<&str>,
    server: Option<String>,
    key: Option<Utf8PathBuf>,
    user: Option<String>,
) -> anyhow::Result<(String, Utf8PathBuf, String)> {
    let file = name.map(load_server).transpose()?;
    let server = server
        .or(file.as_ref().map(|f| f.server.clone()))
        .ok_or_else(|| anyhow::anyhow!("--server is required"))?;
    let key = key
        .or(file.as_ref().and_then(|f| f.key.clone()))
        .ok_or_else(|| anyhow::anyhow!("--key is required"))?;
    let user = user
        .or(file.as_ref().and_then(|f| f.user.clone()))
        .unwrap_or_else(|| DEFAULT_USER.into());
    Ok((server, key, user))
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
