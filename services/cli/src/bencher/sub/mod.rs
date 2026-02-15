use crate::{CliError, parser::CliSub};

mod compose;
mod mock;
mod organization;
mod project;
mod run;
mod sub_cmd;
mod system;
mod user;

pub use compose::DockerError;
use compose::{down::Down, logs::Logs, up::Up};
use mock::Mock;
pub use mock::MockError;
use organization::{member::Member, organization::Organization};
#[cfg(feature = "plus")]
use project::job::Job;
use project::{
    alert::Alert,
    archive::{Archive, ArchiveAction},
    benchmark::Benchmark,
    branch::Branch,
    measure::Measure,
    metric::Metric,
    perf::Perf,
    plot::Plot,
    project::Project,
    report::Report,
    testbed::Testbed,
    threshold::Threshold,
};
pub use project::{archive::ArchiveError, report::ThresholdsError, threshold::ThresholdError};
use run::Run;
pub use run::{RunError, runner::output::Output};
pub use sub_cmd::SubCmd;
#[cfg(feature = "plus")]
use system::runner::Runner;
#[cfg(feature = "plus")]
use system::spec::Spec;
use system::{auth::Auth, server::Server};
use user::{token::Token, user::User};

#[derive(Debug)]
pub enum Sub {
    Run(Box<Run>),
    Mock(Mock),
    Archive(Archive),
    Up(Up),
    Logs(Logs),
    Down(Down),
    Organization(Organization),
    Member(Member),
    #[cfg(feature = "plus")]
    Plan(organization::plan::Plan),
    #[cfg(feature = "plus")]
    Sso(organization::sso::Sso),
    Project(Project),
    Report(Report),
    #[cfg(feature = "plus")]
    Job(Job),
    Perf(Perf),
    Plot(Plot),
    Branch(Branch),
    Testbed(Testbed),
    Benchmark(Benchmark),
    Measure(Measure),
    Metric(Metric),
    Threshold(Threshold),
    Alert(Alert),
    User(User),
    Token(Token),
    #[cfg(feature = "plus")]
    Runner(Runner),
    #[cfg(feature = "plus")]
    Spec(Spec),
    Server(Server),
    Auth(Auth),
}

impl TryFrom<CliSub> for Sub {
    type Error = CliError;

    fn try_from(sub: CliSub) -> Result<Self, Self::Error> {
        Ok(match sub {
            CliSub::Run(run) => Self::Run(Box::new((*run).try_into()?)),
            CliSub::Mock(mock) => Self::Mock(mock.into()),
            CliSub::Archive(archive) => {
                Self::Archive((archive, ArchiveAction::Archive).try_into()?)
            },
            CliSub::Unarchive(unarchive) => {
                Self::Archive((unarchive, ArchiveAction::Unarchive).try_into()?)
            },
            CliSub::Up(up) => Self::Up(up.into()),
            CliSub::Logs(logs) => Self::Logs(logs.into()),
            CliSub::Down(down) => Self::Down(down.into()),
            CliSub::Organization(organization) => Self::Organization(organization.try_into()?),
            CliSub::Member(member) => Self::Member(member.try_into()?),
            #[cfg(feature = "plus")]
            CliSub::Plan(plan) => Self::Plan(plan.try_into()?),
            #[cfg(feature = "plus")]
            CliSub::Sso(sso) => Self::Sso(sso.try_into()?),
            CliSub::Project(project) => Self::Project(project.try_into()?),
            CliSub::Report(report) => Self::Report(report.try_into()?),
            #[cfg(feature = "plus")]
            CliSub::Job(job) => Self::Job(job.try_into()?),
            CliSub::Perf(perf) => Self::Perf(perf.try_into()?),
            CliSub::Plot(plot) => Self::Plot(plot.try_into()?),
            CliSub::Branch(branch) => Self::Branch(branch.try_into()?),
            CliSub::Testbed(testbed) => Self::Testbed(testbed.try_into()?),
            CliSub::Benchmark(benchmark) => Self::Benchmark(benchmark.try_into()?),
            CliSub::Measure(measure) => Self::Measure(measure.try_into()?),
            CliSub::Metric(metric) => Self::Metric(metric.try_into()?),
            CliSub::Threshold(threshold) => Self::Threshold(threshold.try_into()?),
            CliSub::Alert(alert) => Self::Alert(alert.try_into()?),
            CliSub::User(user) => Self::User(user.try_into()?),
            CliSub::Token(token) => Self::Token(token.try_into()?),
            #[cfg(feature = "plus")]
            CliSub::Runner(runner) => Self::Runner(runner.try_into()?),
            #[cfg(feature = "plus")]
            CliSub::Spec(spec) => Self::Spec(spec.try_into()?),
            CliSub::Server(server) => Self::Server(server.try_into()?),
            CliSub::Auth(auth) => Self::Auth(auth.try_into()?),
        })
    }
}

impl SubCmd for Sub {
    #[expect(clippy::cognitive_complexity, reason = "Match arms for each variant")]
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::Run(run) => run.exec().await,
            Self::Mock(mock) => mock.exec().await,
            Self::Archive(archive) => archive.exec().await,
            Self::Up(up) => up.exec().await,
            Self::Logs(logs) => logs.exec().await,
            Self::Down(down) => down.exec().await,
            Self::Organization(organization) => organization.exec().await,
            Self::Member(member) => member.exec().await,
            #[cfg(feature = "plus")]
            Self::Plan(plan) => plan.exec().await,
            #[cfg(feature = "plus")]
            Self::Sso(sso) => sso.exec().await,
            Self::Project(project) => project.exec().await,
            Self::Report(report) => report.exec().await,
            #[cfg(feature = "plus")]
            Self::Job(job) => job.exec().await,
            Self::Perf(perf) => perf.exec().await,
            Self::Plot(plot) => plot.exec().await,
            Self::Branch(branch) => branch.exec().await,
            Self::Testbed(testbed) => testbed.exec().await,
            Self::Benchmark(benchmark) => benchmark.exec().await,
            Self::Measure(measure) => measure.exec().await,
            Self::Metric(metric) => metric.exec().await,
            Self::Threshold(threshold) => threshold.exec().await,
            Self::Alert(alert) => alert.exec().await,
            Self::User(user) => user.exec().await,
            Self::Token(token) => token.exec().await,
            #[cfg(feature = "plus")]
            Self::Runner(runner) => runner.exec().await,
            #[cfg(feature = "plus")]
            Self::Spec(spec) => spec.exec().await,
            Self::Server(server) => server.exec().await,
            Self::Auth(auth) => auth.exec().await,
        }
    }
}
