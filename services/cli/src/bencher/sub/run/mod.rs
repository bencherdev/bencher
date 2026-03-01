use std::{future::Future, pin::Pin};

#[cfg(feature = "plus")]
use std::collections::HashMap;

#[cfg(feature = "plus")]
use bencher_client::types::JsonNewRunJob;
use bencher_client::types::{Adapter, JsonAverage, JsonFold, JsonNewRun, JsonReportSettings};
use bencher_comment::ReportComment;
#[cfg(feature = "plus")]
use bencher_json::SpecResourceId;
use bencher_json::{
    DateTime, JsonReport, ProjectResourceId, RunContext, TestbedNameId, project::report::Iteration,
};

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
mod project;
pub mod runner;
mod sub_adapter;

use branch::Branch;
use ci::Ci;
pub use error::RunError;
use format::Format;
use project::map_project;
use runner::Runner;
use sub_adapter::SubAdapter;

use crate::bencher::SubCmd;

use super::project::report::Thresholds;

#[derive(Debug)]
#[expect(clippy::struct_excessive_bools)]
pub struct Run {
    project: Option<ProjectResourceId>,
    branch: Branch,
    testbed: Option<TestbedNameId>,
    adapter: Adapter,
    sub_adapter: SubAdapter,
    average: Option<JsonAverage>,
    iter: Iteration,
    fold: Option<JsonFold>,
    backdate: Option<DateTime>,
    allow_failure: bool,
    thresholds: Thresholds,
    err: bool,
    format: Format,
    log: bool,
    ci: Option<Ci>,
    runner: Option<Runner>,
    #[expect(clippy::struct_field_names)]
    dry_run: bool,
    #[cfg(feature = "plus")]
    job: Option<Job>,
    backend: PubBackend,
}

#[cfg(feature = "plus")]
#[derive(Debug)]
struct Job {
    image: bencher_json::ImageReference,
    spec: Option<SpecResourceId>,
    entrypoint: Option<String>,
    env: Option<HashMap<String, String>>,
    timeout: Option<bencher_json::Timeout>,
    build_time: bool,
    poll_interval: u64,
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
            #[cfg(feature = "plus")]
            job,
            backend,
        } = run;
        #[cfg(feature = "plus")]
        let build_time = cmd.build_time;
        #[cfg(feature = "plus")]
        let job = if let Some(image) = job.image {
            Some(Job {
                image,
                spec: job.spec,
                entrypoint: job.entrypoint,
                env: job.env.map(bencher_parser::parse_env),
                timeout: job.job_timeout,
                build_time,
                poll_interval: job.job_poll_interval,
            })
        } else {
            None
        };
        #[cfg(feature = "plus")]
        if build_time && job.is_none() && cmd.command.is_none() {
            return Err(RunError::BuildTimeNoCommandOrImage.into());
        }
        #[cfg(not(feature = "plus"))]
        if cmd.build_time && cmd.command.is_none() {
            return Err(RunError::BuildTimeNoCommandOrImage.into());
        }
        let sub_adapter: SubAdapter = (&cmd).into();
        #[cfg(feature = "plus")]
        let runner = if job.is_some() {
            match cmd.try_into() {
                Ok(runner) => Some(runner),
                Err(RunError::NoCommand) => None,
                Err(e) => return Err(e.into()),
            }
        } else {
            Some(cmd.try_into()?)
        };
        #[cfg(not(feature = "plus"))]
        let runner = Some(cmd.try_into()?);
        Ok(Self {
            project: map_project(project)?,
            branch: branch.try_into().map_err(RunError::Branch)?,
            testbed,
            adapter: adapter.into(),
            sub_adapter,
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
            runner,
            dry_run,
            #[cfg(feature = "plus")]
            job,
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

        #[cfg(feature = "plus")]
        if let Some(job_uuid) = json_report.job {
            return self.poll_job(json_report, job_uuid).await;
        }

        let alerts_count = json_report.alerts.len();
        self.display_results(json_report).await?;

        if self.err && alerts_count > 0 {
            Err(RunError::Alerts(alerts_count))
        } else {
            Ok(())
        }
    }

    async fn generate_report(&self) -> Result<Option<JsonNewRun>, RunError> {
        #[cfg(feature = "plus")]
        if let Some(job) = &self.job {
            return Ok(Some(self.generate_remote_report(job)));
        }

        self.generate_local_report().await
    }

    async fn generate_local_report(&self) -> Result<Option<JsonNewRun>, RunError> {
        let runner = self.runner.as_ref().ok_or(RunError::NoRunner)?;
        let start_time = DateTime::now();
        let iter = self.iter.as_usize();
        let mut results = Vec::with_capacity(iter);
        for _ in 0..iter {
            let outputs = runner.run(self.log).await?;
            for output in outputs {
                if output.is_success() {
                    results.push(output.result());
                } else if self.allow_failure {
                    cli_eprintln_quietable!(self.log, "Skipping failure:\n{output}");
                } else {
                    return Err(RunError::ExitStatus {
                        runner: Box::new(runner.clone()),
                        output,
                    });
                }
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
            job: None,
        }))
    }

    #[cfg(feature = "plus")]
    fn generate_remote_report(&self, job: &Job) -> JsonNewRun {
        let cmd = self.runner.as_ref().and_then(Runner::cmd_args);
        let file_paths = self
            .runner
            .as_ref()
            .and_then(Runner::file_paths)
            .map(|paths| paths.into_iter().map(Into::into).collect());
        let file_size = self.runner.as_ref().is_some_and(Runner::file_size);

        let now = DateTime::now();
        let (branch, hash, start_point) = self.branch.clone().into();
        JsonNewRun {
            project: self.project.clone().map(Into::into),
            branch,
            hash,
            start_point,
            testbed: self.testbed.clone().map(Into::into),
            thresholds: self.thresholds.clone().into(),
            start_time: now.into(),
            end_time: now.into(),
            results: Vec::new(),
            settings: Some(JsonReportSettings {
                adapter: Some(self.adapter),
                average: self.average,
                fold: self.fold,
            }),
            context: Some(RunContext::current().into()),
            job: Some(JsonNewRunJob {
                image: job.image.clone().into(),
                spec: job.spec.clone().map(Into::into),
                entrypoint: job.entrypoint.clone().map(|ep| vec![ep]),
                cmd,
                env: job.env.clone(),
                timeout: job.timeout.map(Into::into),
                file_paths,
                build_time: job.build_time.then_some(true),
                file_size: file_size.then_some(true),
                iter: Some(self.iter.into()),
                allow_failure: self.allow_failure.then_some(true),
                backdate: self.backdate.map(Into::into),
            }),
        }
    }

    #[cfg(feature = "plus")]
    async fn poll_job(
        &self,
        json_report: JsonReport,
        job_uuid: bencher_json::JobUuid,
    ) -> Result<(), RunError> {
        use bencher_json::JobStatus;

        let poll_interval = self.job.as_ref().map_or(5, |j| j.poll_interval);
        let job_timeout = self
            .job
            .as_ref()
            .and_then(|j| j.timeout)
            .map_or(3600u64, |t| u64::from(u32::from(t)));
        // CLI-side timeout is 2x the job timeout to allow for queue time
        let cli_timeout = job_timeout.saturating_mul(2);

        let project_resource_id = ProjectResourceId::Slug(json_report.project.slug.clone());

        cli_eprintln_quietable!(self.log, "Waiting for remote job {job_uuid} to complete...");

        let mut last_status: Option<JobStatus> = None;
        let start = std::time::Instant::now();

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(poll_interval)).await;

            if start.elapsed().as_secs() > cli_timeout {
                return Err(RunError::JobTimeout(cli_timeout));
            }

            let json_job: bencher_json::JsonJob = self
                .backend
                .send_with(|client| {
                    let project_resource_id = project_resource_id.clone();
                    async move {
                        client
                            .proj_job_get()
                            .project(project_resource_id)
                            .job(job_uuid)
                            .send()
                            .await
                    }
                })
                .await
                .map_err(RunError::PollJob)?;

            let status = json_job.status;

            // Print status changes
            if !last_status.is_some_and(|ls| ls == status) {
                cli_eprintln_quietable!(self.log, "Job status: {status}");
                last_status = Some(status);
            }

            if !status.has_run() {
                continue;
            }

            // Display stdout/stderr from job output
            if let Some(output) = &json_job.output {
                for result in &output.results {
                    if let Some(stdout) = &result.stdout
                        && !stdout.is_empty()
                    {
                        cli_eprintln_quietable!(self.log, "\nJob stdout:\n{stdout}");
                    }
                    if let Some(stderr) = &result.stderr
                        && !stderr.is_empty()
                    {
                        cli_eprintln_quietable!(self.log, "\nJob stderr:\n{stderr}");
                    }
                }
            }

            match status {
                JobStatus::Completed | JobStatus::Processed => {
                    // Fetch the updated report with results
                    let updated_report: JsonReport = self
                        .backend
                        .send_with(|client| {
                            let project_resource_id = project_resource_id.clone();
                            let report_uuid = json_report.uuid;
                            async move {
                                client
                                    .proj_report_get()
                                    .project(project_resource_id)
                                    .report(report_uuid)
                                    .send()
                                    .await
                            }
                        })
                        .await
                        .map_err(RunError::FetchReport)?;

                    let alerts_count = updated_report.alerts.len();
                    self.display_results(updated_report).await?;

                    return if self.err && alerts_count > 0 {
                        Err(RunError::Alerts(alerts_count))
                    } else {
                        Ok(())
                    };
                },
                JobStatus::Failed => {
                    let error_msg = json_job
                        .output
                        .and_then(|o| o.error)
                        .unwrap_or_else(|| "Unknown error".to_owned());
                    return Err(RunError::JobFailed(error_msg));
                },
                JobStatus::Canceled => {
                    return Err(RunError::JobCanceled);
                },
                // Non-terminal states are handled by the continue above
                JobStatus::Pending | JobStatus::Claimed | JobStatus::Running => {},
            }
        }
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
