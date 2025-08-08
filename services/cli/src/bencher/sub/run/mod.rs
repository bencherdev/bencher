use std::{future::Future, pin::Pin};

use bencher_client::types::{Adapter, JsonAverage, JsonFold, JsonNewRun, JsonReportSettings};
use bencher_comment::ReportComment;
use bencher_json::{DateTime, JsonReport, ResourceId, RunContext, TestbedNameId};

use crate::{
    CliError,
    bencher::backend::PubBackend,
    cli_eprintln_quietable, cli_println, cli_println_quietable,
    parser::run::{CliRun, CliRunOutput},
};

mod branch;
mod ci;
mod error;
mod format;
pub mod runner;
mod sub_adapter;

use branch::Branch;
use ci::Ci;
pub use error::RunError;
use format::Format;
use runner::Runner;
use sub_adapter::SubAdapter;

use crate::bencher::SubCmd;

use super::project::report::Thresholds;

#[derive(Debug)]
#[expect(clippy::struct_excessive_bools)]
pub struct Run {
    project: Option<ResourceId>,
    branch: Branch,
    testbed: Option<TestbedNameId>,
    adapter: Adapter,
    sub_adapter: SubAdapter,
    average: Option<JsonAverage>,
    iter: usize,
    fold: Option<JsonFold>,
    backdate: Option<DateTime>,
    allow_failure: bool,
    thresholds: Thresholds,
    err: bool,
    format: Format,
    log: bool,
    ci: Option<Ci>,
    runner: Runner,
    #[expect(clippy::struct_field_names)]
    dry_run: bool,
    backend: PubBackend,
}

impl TryFrom<CliRun> for Run {
    type Error = CliError;

    fn try_from(run: CliRun) -> Result<Self, Self::Error> {
        let CliRun {
            project,
            branch,
            testbed,
            adapter,
            average,
            iter,
            fold,
            backdate,
            allow_failure,
            thresholds,
            err,
            output: CliRunOutput { format, quiet },
            ci,
            cmd,
            dry_run,
            backend,
        } = run;
        Ok(Self {
            project,
            branch: branch.try_into().map_err(RunError::Branch)?,
            testbed,
            adapter: adapter.into(),
            sub_adapter: (&cmd).into(),
            average: average.map(Into::into),
            iter,
            fold: fold.map(Into::into),
            backdate,
            allow_failure,
            thresholds: thresholds.try_into().map_err(RunError::Thresholds)?,
            err,
            format: format.into(),
            log: !quiet,
            ci: ci.try_into().map_err(RunError::Ci)?,
            runner: cmd.try_into()?,
            dry_run,
            backend: PubBackend::try_from(backend)?.log(false),
        })
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

        let Some(json_new_run) = self.generate_report().await? else {
            return Ok(());
        };

        cli_println_quietable!(self.log, "\nBencher New Report:");
        cli_println_quietable!(
            self.log,
            "{}",
            serde_json::to_string_pretty(&json_new_run).map_err(RunError::SerializeReport)?
        );

        // If performing a dry run, don't actually send the report
        if self.dry_run {
            return Ok(());
        }

        let sender = run_sender(json_new_run);
        let json_report: JsonReport = self
            .backend
            .send_with(sender)
            .await
            .map_err(RunError::SendReport)?;

        let alerts_count = json_report.alerts.len();
        self.display_results(json_report).await?;

        if self.err && alerts_count > 0 {
            Err(RunError::Alerts(alerts_count))
        } else {
            Ok(())
        }
    }

    async fn generate_report(&self) -> Result<Option<JsonNewRun>, RunError> {
        let start_time = DateTime::now();
        let mut results = Vec::with_capacity(self.iter);
        for _ in 0..self.iter {
            let output = self.runner.run(self.log).await?;
            if output.is_success() {
                results.push(output.result());
            } else if self.allow_failure {
                cli_eprintln_quietable!(self.log, "Skipping failure:\n{output}");
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

        let (branch, hash, start_point) = self.branch.clone().into();
        Ok(Some(JsonNewRun {
            project: self.project.clone().map(Into::into),
            branch,
            hash,
            start_point,
            testbed: self.testbed.clone().map(Into::into),
            thresholds: self.thresholds.clone().into(),
            start_time: start_time.into(),
            end_time: end_time.into(),
            results,
            settings: Some(JsonReportSettings {
                adapter: Some(self.adapter),
                average: self.average,
                fold: self.fold,
            }),
            context: Some(RunContext::current().into()),
        }))
    }

    async fn display_results(&self, json_report: JsonReport) -> Result<(), RunError> {
        let console_url = self
            .backend
            .get_console_url()
            .await
            .map_err(RunError::ConsoleUrl)?;
        let source = self
            .ci
            .as_ref()
            .map_or_else(|| "cli".to_owned(), Ci::source);
        let report_comment =
            ReportComment::new(console_url, json_report, self.sub_adapter.into(), source);

        let report_str = match self.format {
            Format::Human => report_comment.human(),
            Format::Json => report_comment.json().map_err(RunError::SerializeReport)?,
            Format::Html => report_comment.html(false, None),
        };
        let newline_prefix = if self.log { "\n" } else { "" };
        cli_println!("{newline_prefix}{report_str}");

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
fn run_sender(
    json_new_run: JsonNewRun,
) -> Box<dyn Fn(bencher_client::Client) -> ReportResult + Send> {
    Box::new(move |client: bencher_client::Client| {
        let json_new_run = json_new_run.clone();
        Box::pin(async move { client.run_post().body(json_new_run.clone()).send().await })
    })
}
