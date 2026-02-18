use clap::Parser as _;

use crate::parser::{TaskSub, TaskTask, TaskTest};

mod clean;
mod oci;
mod scenarios;
mod test;

use clean::Clean;
use oci::Oci;
use scenarios::Scenarios;
use test::Test;

#[derive(Debug)]
pub struct Task {
    sub: Sub,
}

#[derive(Debug)]
pub enum Sub {
    Test(Test),
    Scenarios(Scenarios),
    Oci(Oci),
    Clean(Clean),
}

impl TryFrom<TaskTask> for Task {
    type Error = anyhow::Error;

    fn try_from(task: TaskTask) -> Result<Self, Self::Error> {
        Ok(Self {
            sub: task.sub.unwrap_or(TaskSub::Test(TaskTest {})).try_into()?,
        })
    }
}

impl TryFrom<TaskSub> for Sub {
    type Error = anyhow::Error;

    fn try_from(sub: TaskSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            TaskSub::Test(test) => Self::Test(test.try_into()?),
            TaskSub::Scenarios(scenarios) => Self::Scenarios(scenarios.try_into()?),
            TaskSub::Oci(oci) => Self::Oci(oci.try_into()?),
            TaskSub::Clean(clean) => Self::Clean(clean.try_into()?),
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
            Self::Test(test) => test.exec(),
            Self::Scenarios(scenarios) => scenarios.exec(),
            Self::Oci(oci) => oci.exec(),
            Self::Clean(clean) => clean.exec(),
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
