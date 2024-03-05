use std::{future::Future, pin::Pin};

use bencher_client::types::{Adapter, JsonAverage, JsonFold, JsonNewReport, JsonReportSettings};
use bencher_comment::ReportComment;
use bencher_json::{DateTime, GitHash, JsonConsole, JsonReport, ResourceId};
use clap::ValueEnum;
use url::Url;

use crate::{
    bencher::backend::AuthBackend,
    cli_eprintln_quietable, cli_println, cli_println_quietable,
    parser::project::run::{CliRun, CliRunAdapter},
    CliError,
};

mod adapter;
mod average;
mod branch;
mod ci;
mod error;
mod fold;
pub mod runner;
mod testbed;

use branch::Branch;
use ci::Ci;
pub use error::RunError;
use runner::Runner;
use testbed::Testbed;

use crate::bencher::SubCmd;

pub const BENCHER_PROJECT: &str = "BENCHER_PROJECT";
const BENCHER_BRANCH: &str = "BENCHER_BRANCH";
const BENCHER_TESTBED: &str = "BENCHER_TESTBED";
const BENCHER_ADAPTER: &str = "BENCHER_ADAPTER";
const BENCHER_CMD: &str = "BENCHER_CMD";

#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct Run {
    project: ResourceId,
    branch: Branch,
    hash: Option<GitHash>,
    testbed: Testbed,
    adapter: Option<Adapter>,
    average: Option<JsonAverage>,
    iter: usize,
    fold: Option<JsonFold>,
    backdate: Option<DateTime>,
    allow_failure: bool,
    err: bool,
    html: bool,
    log: bool,
    ci: Option<Ci>,
    runner: Runner,
    #[allow(clippy::struct_field_names)]
    dry_run: bool,
    backend: AuthBackend,
}

impl TryFrom<CliRun> for Run {
    type Error = CliError;

    fn try_from(run: CliRun) -> Result<Self, Self::Error> {
        let CliRun {
            project,
            run_branch,
            hash,
            testbed,
            adapter,
            average,
            iter,
            fold,
            backdate,
            allow_failure,
            err,
            fmt,
            ci,
            cmd,
            dry_run,
            backend,
        } = run;
        Ok(Self {
            project: unwrap_project(project)?,
            branch: run_branch.try_into().map_err(RunError::Branch)?,
            hash: map_hash(hash),
            testbed: testbed.try_into().map_err(RunError::Testbed)?,
            adapter: map_adapter(adapter),
            average: average.map(Into::into),
            iter: iter.unwrap_or(1),
            fold: fold.map(Into::into),
            backdate,
            allow_failure,
            err,
            html: fmt.html,
            log: !fmt.quiet,
            ci: ci.try_into().map_err(RunError::Ci)?,
            runner: cmd.try_into()?,
            dry_run,
            backend: AuthBackend::try_from(backend)?.log(false),
        })
    }
}

fn unwrap_project(project: Option<ResourceId>) -> Result<ResourceId, RunError> {
    Ok(if let Some(project) = project {
        project
    } else if let Ok(env_project) = std::env::var(BENCHER_PROJECT) {
        env_project.parse().map_err(RunError::ParseProject)?
    } else {
        return Err(RunError::NoProject);
    })
}

fn map_hash(hash: Option<GitHash>) -> Option<GitHash> {
    if let Some(hash) = hash {
        return Some(hash);
    }

    let current_dir = std::env::current_dir().ok()?;
    for directory in current_dir.ancestors() {
        let Some(repo) = gix::open(directory).ok() else {
            continue;
        };
        let head_id = repo.head_id().ok()?;
        let head_object = head_id.object().ok()?;
        return Some(head_object.id.into());
    }

    None
}

fn map_adapter(adapter: Option<CliRunAdapter>) -> Option<Adapter> {
    if let Some(adapter) = adapter {
        Some(adapter.into())
    } else if let Ok(env_adapter) = std::env::var(BENCHER_ADAPTER) {
        if let Ok(cli_adapter) = CliRunAdapter::from_str(&env_adapter, false) {
            Some(cli_adapter.into())
        } else {
            None
        }
    } else {
        None
    }
}

impl SubCmd for Run {
    async fn exec(&self) -> Result<(), CliError> {
        self.exec_inner().await.map_err(Into::into)
    }
}

impl Run {
    async fn exec_inner(&self) -> Result<(), RunError> {
        if let Some(mismatch) = self
            .backend
            .check_version()
            .await
            .map_err(RunError::ApiVersion)?
        {
            cli_eprintln_quietable!(self.log, "Warning: {mismatch}");
        }

        if let Some(ci) = &self.ci {
            ci.safety_check(self.log)?;
        }

        let Some(json_new_report) = self.generate_report().await? else {
            return Ok(());
        };

        cli_println_quietable!(self.log, "\nBencher New Report:");
        cli_println_quietable!(
            self.log,
            "{}",
            serde_json::to_string_pretty(&json_new_report).map_err(RunError::SerializeReport)?
        );

        // If performing a dry run, don't actually send the report
        if self.dry_run {
            return Ok(());
        }

        let sender = report_sender(self.project.clone(), json_new_report);
        // If we are not doing complex output logging then we don't need to a strict deserialization.
        if !self.log {
            let json_report = self
                .backend
                .send(sender)
                .await
                .map_err(RunError::SendReport)?;
            return serde_json::to_string_pretty(&json_report)
                .map(|json| cli_println!("{json}"))
                .map_err(RunError::SerializeReport);
        }

        cli_println!("\nBencher Report:");
        let json_report: JsonReport = self
            .backend
            .send_with(sender)
            .await
            .map_err(RunError::SendReport)?;
        if let Ok(json) = serde_json::to_string_pretty(&json_report) {
            cli_println!("{json}");
        }

        let alerts_count = json_report.alerts.len();
        self.display_results(json_report).await?;

        if self.err && alerts_count > 0 {
            Err(RunError::Alerts(alerts_count))
        } else {
            Ok(())
        }
    }

    async fn generate_report(&self) -> Result<Option<JsonNewReport>, RunError> {
        let Some(branch) = self
            .branch
            .get(&self.project, self.dry_run, self.log, &self.backend)
            .await?
        else {
            return Ok(None);
        };
        let testbed = self
            .testbed
            .get(&self.project, self.dry_run, &self.backend)
            .await?;

        let start_time = DateTime::now();
        let mut results = Vec::with_capacity(self.iter);
        for _ in 0..self.iter {
            let output = self.runner.run(self.log).await?;
            if output.is_success() {
                results.push(output.result());
            } else if self.allow_failure {
                cli_eprintln_quietable!(self.log, "Skipping failure:\n{}", output);
            } else {
                return Err(RunError::ExitStatus {
                    runner: Box::new(self.runner.clone()),
                    output,
                });
            }
        }

        cli_println_quietable!(self.log, "\nBenchmark Harness Results:");
        for result in &results {
            cli_println_quietable!(self.log, "{result}");
        }

        let end_time = DateTime::now();
        // If a backdate is set then use it as the start time and calculate the end time from there
        let (start_time, end_time) = if let Some(backdate) = self.backdate {
            let elapsed = end_time.into_inner() - start_time.into_inner();
            (backdate, DateTime::from(backdate.into_inner() + elapsed))
        } else {
            (start_time, end_time)
        };

        Ok(Some(JsonNewReport {
            branch: branch.into(),
            hash: self.hash.clone().map(Into::into),
            testbed: testbed.into(),
            start_time: start_time.into(),
            end_time: end_time.into(),
            results,
            settings: Some(JsonReportSettings {
                adapter: self.adapter,
                average: self.average,
                fold: self.fold,
            }),
        }))
    }

    async fn display_results(&self, json_report: JsonReport) -> Result<(), RunError> {
        let json_console: JsonConsole = self
            .backend
            .send_with(|client| async move { client.server_config_console_get().send().await })
            .await
            .map_err(RunError::GetEndpoint)?;
        let console_url: Url = json_console.url.try_into().map_err(RunError::BadEndpoint)?;
        let report_comment = ReportComment::new(console_url, json_report);

        if self.html {
            let with_metrics = true;
            let require_threshold = false;
            cli_println!(
                "{}",
                report_comment.html(with_metrics, require_threshold, None)
            );
        } else {
            cli_println!("{}", report_comment.text());
        }

        if let Some(ci) = &self.ci {
            ci.run(&report_comment, self.log).await?;
        }

        Ok(())
    }
}

type ReportResult = Pin<
    Box<
        dyn Future<
                Output = Result<
                    progenitor_client::ResponseValue<bencher_client::types::JsonReport>,
                    bencher_client::Error<bencher_client::types::Error>,
                >,
            > + Send,
    >,
>;
fn report_sender(
    project: ResourceId,
    json_new_report: JsonNewReport,
) -> Box<dyn Fn(bencher_client::Client) -> ReportResult + Send> {
    Box::new(move |client: bencher_client::Client| {
        let project = project.clone();
        let json_new_report = json_new_report.clone();
        Box::pin(async move {
            client
                .proj_report_post()
                .project(project.clone())
                .body(json_new_report.clone())
                .send()
                .await
        })
    })
}
