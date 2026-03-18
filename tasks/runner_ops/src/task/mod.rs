mod deploy;
mod download;
mod harden;
mod install_os;
mod logs;
mod provision;
mod setup;
pub mod ssh;
mod start;
pub mod stop;

use camino::Utf8PathBuf;
use clap::Parser as _;

use crate::parser::server::load_server;
use crate::parser::{TaskRunnerOps, TaskSub};
use deploy::Deploy;
use logs::Logs;
use provision::Provision;
use ssh::Ssh;
use start::Start;
use stop::Stop;

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

/// Merge SSH + runner/token/host fields from CLI flags and optional server config file.
fn merge_ssh_with_extras(
    name: Option<&str>,
    server: Option<String>,
    key: Option<Utf8PathBuf>,
    user: Option<String>,
    runner: Option<String>,
    token: Option<String>,
    host: Option<url::Url>,
) -> anyhow::Result<(Ssh, url::Url, String, String)> {
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
    let runner = runner
        .or(file.as_ref().and_then(|f| f.runner.clone()))
        .ok_or_else(|| anyhow::anyhow!("--runner is required"))?;
    let token = token
        .or(file.as_ref().and_then(|f| f.token.clone()))
        .ok_or_else(|| anyhow::anyhow!("--token is required"))?;
    let host = host
        .or(file.as_ref().and_then(|f| f.host.clone()))
        .unwrap_or_else(|| DEFAULT_HOST.clone());
    Ok((Ssh::new(server, key, user), host, runner, token))
}
