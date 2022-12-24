use std::convert::TryFrom;

use async_trait::async_trait;

use crate::{cli::CliSub, CliError};

mod mock;
mod organization;
mod project;
mod sub_cmd;
mod system;
mod user;

use mock::Mock;
use organization::{member::Member, resource::Organization};
use project::{
    alert::Alert, benchmark::Benchmark, branch::Branch, metric_kind::MetricKind, perf::Perf,
    report::Report, resource::Project, result::Resultant, run::Run, testbed::Testbed,
    threshold::Threshold,
};
pub use sub_cmd::SubCmd;
use system::{auth::Auth, docs::Docs, server::Server};
use user::resource::User;
use user::token::Token;

#[derive(Debug)]
pub enum Sub {
    Server(Server),
    Auth(Auth),
    Organization(Organization),
    Member(Member),
    Project(Project),
    Report(Report),
    Result(Resultant),
    Branch(Branch),
    Testbed(Testbed),
    Threshold(Threshold),
    MetricKind(MetricKind),
    Run(Run),
    Benchmark(Benchmark),
    Perf(Perf),
    Alert(Alert),
    User(User),
    Token(Token),
    Mock(Mock),
    Docs(Docs),
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
            CliSub::Result(result) => Self::Result(result.try_into()?),
            CliSub::Branch(branch) => Self::Branch(branch.try_into()?),
            CliSub::Testbed(testbed) => Self::Testbed(testbed.try_into()?),
            CliSub::Threshold(threshold) => Self::Threshold(threshold.try_into()?),
            CliSub::MetricKind(metric_kind) => Self::MetricKind(metric_kind.try_into()?),
            CliSub::Run(run) => Self::Run(run.try_into()?),
            CliSub::Benchmark(benchmark) => Self::Benchmark(benchmark.try_into()?),
            CliSub::Perf(perf) => Self::Perf(perf.try_into()?),
            CliSub::Alert(alert) => Self::Alert(alert.try_into()?),
            CliSub::User(user) => Self::User(user.try_into()?),
            CliSub::Token(token) => Self::Token(token.try_into()?),
            CliSub::Mock(mock) => Self::Mock(mock.into()),
            CliSub::Docs(docs) => Self::Docs(docs.into()),
        })
    }
}

#[async_trait]
impl SubCmd for Sub {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::Server(server) => server.exec().await,
            Self::Auth(auth) => auth.exec().await,
            Self::Organization(organization) => organization.exec().await,
            Self::Member(member) => member.exec().await,
            Self::Project(project) => project.exec().await,
            Self::Report(report) => report.exec().await,
            Self::Result(result) => result.exec().await,
            Self::Branch(branch) => branch.exec().await,
            Self::Testbed(testbed) => testbed.exec().await,
            Self::Threshold(threshold) => threshold.exec().await,
            Self::MetricKind(metric_kind) => metric_kind.exec().await,
            Self::Run(run) => run.exec().await,
            Self::Benchmark(benchmark) => benchmark.exec().await,
            Self::Perf(perf) => perf.exec().await,
            Self::Alert(alert) => alert.exec().await,
            Self::User(user) => user.exec().await,
            Self::Token(token) => token.exec().await,
            Self::Mock(mock) => mock.exec().await,
            Self::Docs(docs) => docs.exec().await,
        }
    }
}
