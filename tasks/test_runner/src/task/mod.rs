mod scenarios;

use clap::Parser as _;

use crate::parser::{TaskSub, TaskTask};
use scenarios::Scenarios;

#[derive(Debug)]
pub struct Task {
    sub: Sub,
}

#[derive(Debug)]
pub enum Sub {
    Scenarios(Scenarios),
}

impl TryFrom<TaskTask> for Task {
    type Error = anyhow::Error;

    fn try_from(task: TaskTask) -> Result<Self, Self::Error> {
        Ok(Self {
            sub: task.sub.try_into()?,
        })
    }
}

impl TryFrom<TaskSub> for Sub {
    type Error = anyhow::Error;

    fn try_from(sub: TaskSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            TaskSub::Scenarios(scenarios) => Self::Scenarios(scenarios.try_into()?),
        })
    }
}

impl Task {
    pub fn new() -> anyhow::Result<Self> {
        TaskTask::parse().try_into()
    }

    pub fn exec(&self) -> anyhow::Result<()> {
        self.sub.exec()
    }
}

impl Sub {
    pub fn exec(&self) -> anyhow::Result<()> {
        match self {
            Self::Scenarios(scenarios) => scenarios.exec(),
        }
    }
}

/// Get the workspace root directory.
pub(super) fn workspace_root() -> camino::Utf8PathBuf {
    camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("Failed to find workspace root")
        .to_owned()
}

/// Map `std::env::consts::ARCH` to Rust target triples for musl builds.
pub(super) fn musl_target_triple() -> anyhow::Result<&'static str> {
    use std::env::consts::ARCH;
    match ARCH {
        "x86_64" => Ok("x86_64-unknown-linux-musl"),
        "aarch64" => Ok("aarch64-unknown-linux-musl"),
        arch => anyhow::bail!("Unsupported architecture: {arch}"),
    }
}

/// Get the work directory for test artifacts.
pub fn work_dir() -> camino::Utf8PathBuf {
    let dir = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-runner");
    std::fs::create_dir_all(&dir).expect("Failed to create work directory");
    dir
}
