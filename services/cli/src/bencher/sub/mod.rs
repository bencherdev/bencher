use std::convert::TryFrom;

use async_trait::async_trait;

use crate::{
    bencher::wide::Wide,
    cli::CliSub,
    BencherError,
};

mod auth;
mod benchmark;
mod branch;
mod perf;
mod project;
mod report;
mod run;
mod subcmd;
mod testbed;

use auth::Auth;
use benchmark::Benchmark;
use branch::Branch;
use perf::Perf;
use project::Project;
use report::Report;
use run::Run;
pub use subcmd::SubCmd;
use testbed::Testbed;

#[derive(Debug)]
pub enum Sub {
    Auth(Auth),
    Project(Project),
    Report(Report),
    Branch(Branch),
    Testbed(Testbed),
    Benchmark(Benchmark),
    Run(Run),
    Perf(Perf),
}

impl TryFrom<CliSub> for Sub {
    type Error = BencherError;

    fn try_from(sub: CliSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            CliSub::Auth(auth) => Self::Auth(Auth::try_from(auth)?),
            CliSub::Project(project) => Self::Project(Project::try_from(project)?),
            CliSub::Report(report) => Self::Report(Report::try_from(report)?),
            CliSub::Branch(branch) => Self::Branch(Branch::try_from(branch)?),
            CliSub::Testbed(testbed) => Self::Testbed(Testbed::try_from(testbed)?),
            CliSub::Benchmark(benchmark) => Self::Benchmark(Benchmark::try_from(benchmark)?),
            CliSub::Run(run) => Self::Run(Run::try_from(run)?),
            CliSub::Perf(perf) => Self::Perf(Perf::try_from(perf)?),
        })
    }
}

pub fn map_sub(sub: Option<CliSub>) -> Result<Option<Sub>, BencherError> {
    if let Some(sub) = sub {
        Ok(Some(Sub::try_from(sub)?))
    } else {
        Ok(None)
    }
}

#[async_trait]
impl SubCmd for Sub {
    async fn exec(&self, wide: &Wide) -> Result<(), BencherError> {
        match self {
            Self::Auth(auth) => auth.exec(wide).await,
            Self::Project(project) => project.exec(wide).await,
            Self::Report(report) => report.exec(wide).await,
            Self::Branch(branch) => branch.exec(wide).await,
            Self::Testbed(testbed) => testbed.exec(wide).await,
            Self::Benchmark(benchmark) => benchmark.exec(wide).await,
            Self::Run(run) => run.exec(wide).await,
            Self::Perf(perf) => perf.exec(wide).await,
        }
    }
}
