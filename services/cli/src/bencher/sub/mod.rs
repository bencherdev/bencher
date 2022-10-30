use std::convert::TryFrom;

use async_trait::async_trait;

use crate::{bencher::wide::Wide, cli::CliSub, CliError};

mod organization;
mod project;
mod sub_cmd;
mod system;
mod user;

use organization::{member::Member, resource::Organization};
use project::{
    alert::Alert, benchmark::Benchmark, branch::Branch, perf::Perf, report::Report,
    resource::Project, run::Run, testbed::Testbed, threshold::Threshold,
};
pub use sub_cmd::SubCmd;
use system::{auth::Auth, server::Server};
use user::token::Token;

#[derive(Debug)]
pub enum Sub {
    Server(Server),
    Auth(Auth),
    Organization(Organization),
    Member(Member),
    Project(Project),
    Report(Report),
    Branch(Branch),
    Testbed(Testbed),
    Threshold(Threshold),
    Run(Run),
    Benchmark(Benchmark),
    Perf(Perf),
    Alert(Alert),
    Token(Token),
}

impl TryFrom<CliSub> for Sub {
    type Error = CliError;

    fn try_from(sub: CliSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            CliSub::Server(server) => Self::Server(server.try_into()?),
            CliSub::Auth(auth) => Self::Auth(auth.try_into()?),
            CliSub::Organization(organization) => Self::Organization(organization.try_into()?),
            CliSub::Member(member) => Self::Member(member.try_into()?),
            CliSub::Project(project) => Self::Project(project.try_into()?),
            CliSub::Report(report) => Self::Report(report.try_into()?),
            CliSub::Branch(branch) => Self::Branch(branch.try_into()?),
            CliSub::Testbed(testbed) => Self::Testbed(testbed.try_into()?),
            CliSub::Threshold(threshold) => Self::Threshold(threshold.try_into()?),
            CliSub::Run(run) => Self::Run(run.try_into()?),
            CliSub::Benchmark(benchmark) => Self::Benchmark(benchmark.try_into()?),
            CliSub::Perf(perf) => Self::Perf(perf.try_into()?),
            CliSub::Alert(alert) => Self::Alert(alert.try_into()?),
            CliSub::Token(token) => Self::Token(token.try_into()?),
        })
    }
}

pub fn map_sub(sub: Option<CliSub>) -> Result<Option<Sub>, CliError> {
    if let Some(sub) = sub {
        Ok(Some(sub.try_into()?))
    } else {
        Ok(None)
    }
}

#[async_trait]
impl SubCmd for Sub {
    async fn exec(&self, wide: &Wide) -> Result<(), CliError> {
        match self {
            Self::Server(server) => server.exec(wide).await,
            Self::Auth(auth) => auth.exec(wide).await,
            Self::Organization(organization) => organization.exec(wide).await,
            Self::Member(member) => member.exec(wide).await,
            Self::Project(project) => project.exec(wide).await,
            Self::Report(report) => report.exec(wide).await,
            Self::Branch(branch) => branch.exec(wide).await,
            Self::Testbed(testbed) => testbed.exec(wide).await,
            Self::Threshold(threshold) => threshold.exec(wide).await,
            Self::Run(run) => run.exec(wide).await,
            Self::Benchmark(benchmark) => benchmark.exec(wide).await,
            Self::Perf(perf) => perf.exec(wide).await,
            Self::Alert(alert) => alert.exec(wide).await,
            Self::Token(token) => token.exec(wide).await,
        }
    }
}
