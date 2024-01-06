use std::convert::TryFrom;

use crate::{parser::CliSub, CliError};

mod docker;
mod mock;
mod organization;
mod project;
mod sub_cmd;
mod system;
mod user;

pub use docker::DockerError;
use docker::{down::Down, up::Up};
use mock::Mock;
pub use mock::MockError;
use organization::{member::Member, organization::Organization};
pub use project::run::{runner::output::Output, RunError};
use project::{
    alert::Alert, benchmark::Benchmark, branch::Branch, measure::Measure, perf::Perf,
    project::Project, report::Report, run::Run, statistic::Statistic, testbed::Testbed,
    threshold::Threshold,
};
pub use sub_cmd::SubCmd;
use system::{auth::Auth, server::Server};
use user::resource::User;
use user::token::Token;

#[derive(Debug)]
pub enum Sub {
    Auth(Auth),
    Organization(Organization),
    Member(Member),
    #[cfg(feature = "plus")]
    Plan(organization::plan::Plan),
    Project(Project),
    Run(Box<Run>),
    Report(Report),
    Perf(Perf),
    Branch(Branch),
    Testbed(Testbed),
    Benchmark(Benchmark),
    Measure(Measure),
    Threshold(Threshold),
    Statistic(Statistic),
    Alert(Alert),
    User(User),
    Token(Token),
    Server(Server),
    Mock(Mock),
    Up(Up),
    Down(Down),
}

impl TryFrom<CliSub> for Sub {
    type Error = CliError;

    fn try_from(sub: CliSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            CliSub::Auth(auth) => Self::Auth(auth.try_into()?),
            CliSub::Organization(organization) => Self::Organization(organization.try_into()?),
            CliSub::Member(member) => Self::Member(member.try_into()?),
            #[cfg(feature = "plus")]
            CliSub::Plan(plan) => Self::Plan(plan.try_into()?),
            CliSub::Project(project) => Self::Project(project.try_into()?),
            CliSub::Run(run) => Self::Run(Box::new((*run).try_into()?)),
            CliSub::Report(report) => Self::Report(report.try_into()?),
            CliSub::Perf(perf) => Self::Perf(perf.try_into()?),
            CliSub::Branch(branch) => Self::Branch(branch.try_into()?),
            CliSub::Testbed(testbed) => Self::Testbed(testbed.try_into()?),
            CliSub::Benchmark(benchmark) => Self::Benchmark(benchmark.try_into()?),
            CliSub::Measure(measure) => Self::Measure(measure.try_into()?),
            CliSub::Threshold(threshold) => Self::Threshold(threshold.try_into()?),
            CliSub::Statistic(statistic) => Self::Statistic(statistic.try_into()?),
            CliSub::Alert(alert) => Self::Alert(alert.try_into()?),
            CliSub::User(user) => Self::User(user.try_into()?),
            CliSub::Token(token) => Self::Token(token.try_into()?),
            CliSub::Server(server) => Self::Server(server.try_into()?),
            CliSub::Mock(mock) => Self::Mock(mock.into()),
            CliSub::Up(up) => Self::Up(up.into()),
            CliSub::Down(down) => Self::Down(down.into()),
        })
    }
}

impl SubCmd for Sub {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::Auth(auth) => auth.exec().await,
            Self::Organization(organization) => organization.exec().await,
            Self::Member(member) => member.exec().await,
            #[cfg(feature = "plus")]
            Self::Plan(plan) => plan.exec().await,
            Self::Project(project) => project.exec().await,
            Self::Run(run) => run.exec().await,
            Self::Report(report) => report.exec().await,
            Self::Perf(perf) => perf.exec().await,
            Self::Branch(branch) => branch.exec().await,
            Self::Testbed(testbed) => testbed.exec().await,
            Self::Benchmark(benchmark) => benchmark.exec().await,
            Self::Measure(measure) => measure.exec().await,
            Self::Threshold(threshold) => threshold.exec().await,
            Self::Statistic(statistic) => statistic.exec().await,
            Self::Alert(alert) => alert.exec().await,
            Self::User(user) => user.exec().await,
            Self::Token(token) => token.exec().await,
            Self::Server(server) => server.exec().await,
            Self::Mock(mock) => mock.exec().await,
            Self::Up(up) => up.exec().await,
            Self::Down(down) => down.exec().await,
        }
    }
}
