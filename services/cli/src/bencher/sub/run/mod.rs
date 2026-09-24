use std::{future::Future, pin::Pin};

#[cfg(feature = "plus")]
use bencher_client::types::JsonNewRunJob;
use bencher_client::types::{Adapter, JsonAverage, JsonFold, JsonNewRun, JsonReportSettings};
use bencher_comment::ReportComment;
use bencher_json::{
    DateTime, JsonProject, JsonReport, ProjectResourceId, ResourceName, RunContext, TestbedNameId,
    project::report::Iteration,
};
#[cfg(feature = "plus")]
use bencher_json::{JobUuid, JsonJob, JsonNewCallback};

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
#[cfg(feature = "plus")]
mod job;
mod project;
pub mod runner;
mod sub_adapter;

use branch::Branch;
use ci::{Ci, CiCheck};
pub use error::RunError;
use format::Format;
#[cfg(feature = "plus")]
use job::{
    AttachJob, CallbackNotice, DEFAULT_POLL_INTERVAL, FinishedJob, Job, JobWait, SubmitJob,
    client_callback, echo_callback,
};
use project::resolve_project;
use runner::Runner;
use sub_adapter::SubAdapter;

use crate::bencher::SubCmd;

use super::project::report::Thresholds;

#[derive(Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each bool is an independent user flag"
)]
pub struct Run {
    project: Option<ProjectResourceId>,
    branch: Branch,
    testbed: Option<TestbedNameId>,
    #[cfg(feature = "plus")]
    spec_reset: bool,
    adapter: Adapter,
    sub_adapter: SubAdapter,
    average: Option<JsonAverage>,
    iter: Iteration,
    fold: Option<JsonFold>,
    backdate: Option<DateTime>,
    allow_failure: bool,
    thresholds: Thresholds,
    error_on_alert: bool,
    format: Format,
    log: bool,
    ci: Option<Ci>,
    runner: Option<Runner>,
    #[expect(
        clippy::struct_field_names,
        reason = "dry_run is the canonical name for this flag"
    )]
    dry_run: bool,
    #[cfg(feature = "plus")]
    job: Option<Job>,
    backend: PubBackend,
}

impl TryFrom<CliRun> for Run {
    type Error = CliError;

    fn try_from(run: CliRun) -> Result<Self, Self::Error> {
        let CliRun {
            project,
            branch,
            testbed,
            #[cfg(feature = "plus")]
            spec_reset,
            adapter,
            average,
            iter,
            fold,
            backdate,
            allow_failure,
            thresholds,
            error_on_alert,
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
        let job = Job::new(job, project.project.as_ref(), build_time)?;
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
        let runner = match job {
            Some(Job::Submit(_)) if cmd.has_local_input() => match cmd.try_into() {
                Ok(runner) => Some(runner),
                Err(RunError::NoCommand) => None,
                Err(e) => return Err(e.into()),
            },
            Some(Job::Submit(_) | Job::Attach(_)) => None,
            None => Some(cmd.try_into()?),
        };
        #[cfg(not(feature = "plus"))]
        let runner = Some(cmd.try_into()?);

        // The server derives the target project from the job image repository
        // (`[{registry}/]{project}:{tag}`) when `--project` is not specified.
        // Mirror that derivation here to relax the `--project` requirements.
        #[cfg(feature = "plus")]
        let has_image_project = matches!(&job, Some(Job::Submit(job)) if job
            .image
            .project_repository()
            .is_some_and(|repository| repository.parse::<ProjectResourceId>().is_ok()));
        #[cfg(not(feature = "plus"))]
        let has_image_project = false;

        Ok(Self {
            project: resolve_project(project, backend.key.as_ref(), has_image_project)?,
            branch: branch.try_into().map_err(RunError::Branch)?,
            testbed,
            #[cfg(feature = "plus")]
            spec_reset,
            adapter: adapter.into(),
            sub_adapter,
            average: average.map(Into::into),
            iter,
            fold: fold.map(Into::into),
            backdate,
            allow_failure,
            thresholds: thresholds.try_into().map_err(RunError::Thresholds)?,
            error_on_alert,
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

        // Start the in-progress CI check before the benchmark runs,
        // so a rerun immediately clears the stale conclusion left by a previous run.
        // Dry runs never post results, so they never start a check.
        let mut ci_check = match &self.ci {
            Some(ci) if !self.dry_run => {
                let project_name = if ci.needs_project_name() {
                    self.project_name().await
                } else {
                    None
                };
                ci.start(project_name.as_ref(), self.log).await
            },
            _ => None,
        };

        let result = self.run_and_report(&mut ci_check).await;
        // Every path that posts results consumes the check handle,
        // so an unconsumed handle means the check was never completed.
        // Best-effort: mark it as failed rather than leave it in progress forever.
        // Today only error paths leave the handle unconsumed. Any future path
        // that returns `Ok` without posting results will mark the check as
        // failed here, which is still better than an eternally pending check.
        if let (Some(ci), Some(check)) = (&self.ci, ci_check.take()) {
            ci.fail(&check, self.log).await;
        }
        result
    }

    /// Best-effort: look up the Project name so the CI check can be named for it.
    /// The name is only knowable before the benchmarks run if `--project` is set,
    /// so an on-the-fly Project simply goes without the Project name in the check name.
    async fn project_name(&self) -> Option<ResourceName> {
        let project = self.project.clone()?;
        let json_project: JsonProject = self
            .backend
            .send_with(|client| {
                let project = project.clone();
                async move { client.project_get().project(project).send().await }
            })
            .await
            .inspect_err(|err| {
                // Expected on the very first run of an on-the-fly Project,
                // which is only created once the Report is posted.
                cli_eprintln_quietable!(
                    self.log,
                    "Could not resolve the Project name for the CI check: {err}"
                );
            })
            .ok()?;
        Some(json_project.name)
    }

    async fn run_and_report(&self, ci_check: &mut Option<CiCheck>) -> Result<(), RunError> {
        #[cfg(feature = "plus")]
        if let Some(Job::Attach(AttachJob {
            project,
            uuid,
            wait,
        })) = &self.job
        {
            return self.wait_for_job(project, *uuid, *wait, ci_check).await;
        }

        // The one callback the run sends, prints, and reads back.
        #[cfg(feature = "plus")]
        let callback = self.callback();

        let Some(json_new_run) = self
            .generate_report(
                #[cfg(feature = "plus")]
                callback,
            )
            .await?
        else {
            return Ok(());
        };

        cli_println_quietable!(self.log, "\nBencher New Report:");
        cli_println_quietable!(
            self.log,
            "{}",
            Self::new_report_echo(
                &json_new_run,
                #[cfg(feature = "plus")]
                callback,
            )?
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
            if self.submit_job().is_some_and(|job| job.detach) {
                cli_eprintln_quietable!(self.log, "Remote job submitted successfully: {job_uuid}");
                if callback.is_some() {
                    self.callback_notice(&json_report, job_uuid).await;
                }
                return self.display_and_check_alerts(json_report, ci_check).await;
            }
            return self.poll_job(json_report, job_uuid, ci_check).await;
        }

        self.display_and_check_alerts(json_report, ci_check).await
    }

    async fn generate_report(
        &self,
        #[cfg(feature = "plus")] callback: Option<&JsonNewCallback>,
    ) -> Result<Option<JsonNewRun>, RunError> {
        #[cfg(feature = "plus")]
        if let Some(job) = self.submit_job() {
            return self.generate_remote_report(job, callback).map(Some);
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
            idempotency_key: Some(uuid::Uuid::new_v4().into()),
            branch,
            hash,
            start_point,
            testbed: self.testbed.clone().map(Into::into),
            spec_reset: self.spec_reset(),
            thresholds: self.thresholds.clone().into(),
            start_time: start_time.into(),
            end_time: end_time.into(),
            results,
            // `bencher run` declares no BMF version yet, so the results are read at
            // the project's default.
            bmf_version: None,
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
    fn generate_remote_report(
        &self,
        job: &SubmitJob,
        callback: Option<&JsonNewCallback>,
    ) -> Result<JsonNewRun, RunError> {
        let cmd = self.runner.as_ref().and_then(Runner::cmd_args);
        let file_paths = self
            .runner
            .as_ref()
            .and_then(Runner::file_paths)
            .map(|paths| paths.into_iter().map(Into::into).collect());
        let file_size = self.runner.as_ref().is_some_and(Runner::file_size);

        let callback = callback.map(client_callback).transpose()?;

        let now = DateTime::now();
        let (branch, hash, start_point) = self.branch.clone().into();
        Ok(JsonNewRun {
            project: self.project.clone().map(Into::into),
            idempotency_key: Some(uuid::Uuid::new_v4().into()),
            branch,
            hash,
            start_point,
            testbed: self.testbed.clone().map(Into::into),
            spec_reset: self.spec_reset(),
            thresholds: self.thresholds.clone().into(),
            start_time: now.into(),
            end_time: now.into(),
            results: Vec::new(),
            bmf_version: None,
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
                callback,
            }),
        })
    }

    /// The new report as printed: the job carries the callback it is handed, through `sanitize_json`,
    /// never the one on the run it is about to send.
    fn new_report_echo(
        json_new_run: &JsonNewRun,
        #[cfg(feature = "plus")] callback: Option<&JsonNewCallback>,
    ) -> Result<String, RunError> {
        #[cfg(feature = "plus")]
        if json_new_run.job.is_some() {
            let mut echo = json_new_run.clone();
            if let Some(job) = &mut echo.job {
                job.callback = callback.map(echo_callback).transpose()?;
            }
            return serde_json::to_string_pretty(&echo).map_err(RunError::SerializeReport);
        }
        serde_json::to_string_pretty(json_new_run).map_err(RunError::SerializeReport)
    }

    /// The attach cannot see the submitting run's `--build-time` and `--file-size`,
    /// so its comment tag follows the built-in measures of the report instead.
    fn comment_sub_adapter(&self, json_report: &JsonReport) -> bencher_comment::SubAdapter {
        if self.is_attach() {
            json_report.into()
        } else {
            self.sub_adapter.into()
        }
    }

    fn is_attach(&self) -> bool {
        #[cfg(feature = "plus")]
        {
            matches!(self.job, Some(Job::Attach(_)))
        }
        #[cfg(not(feature = "plus"))]
        {
            false
        }
    }

    fn spec_reset(&self) -> Option<bool> {
        #[cfg(feature = "plus")]
        {
            self.spec_reset.then_some(true)
        }
        #[cfg(not(feature = "plus"))]
        {
            None
        }
    }

    #[cfg(feature = "plus")]
    fn submit_job(&self) -> Option<&SubmitJob> {
        if let Some(Job::Submit(job)) = &self.job {
            Some(job)
        } else {
            None
        }
    }

    #[cfg(feature = "plus")]
    fn callback(&self) -> Option<&JsonNewCallback> {
        self.submit_job()?.callback.as_deref()
    }

    /// Read the detached job once, and say so if the server skipped its callback.
    /// The job was submitted either way, so a failed read prints no notice.
    #[cfg(feature = "plus")]
    async fn callback_notice(&self, json_report: &JsonReport, job_uuid: JobUuid) {
        let project = ProjectResourceId::Slug(json_report.project.slug.clone());
        let Ok(json_job) = self.get_job(&project, job_uuid).await else {
            return;
        };
        if let Some(notice) = CallbackNotice::new(&json_job) {
            cli_eprintln_quietable!(self.log, "{notice}");
        }
    }

    #[cfg(feature = "plus")]
    async fn poll_job(
        &self,
        json_report: JsonReport,
        job_uuid: JobUuid,
        ci_check: &mut Option<CiCheck>,
    ) -> Result<(), RunError> {
        let wait = self.submit_job().map_or_else(
            || JobWait::submitted(*DEFAULT_POLL_INTERVAL, None),
            |job| job.wait,
        );
        let project = ProjectResourceId::Slug(json_report.project.slug);
        self.wait_for_job(&project, job_uuid, wait, ci_check).await
    }

    #[cfg(feature = "plus")]
    async fn wait_for_job(
        &self,
        project: &ProjectResourceId,
        job_uuid: JobUuid,
        wait: JobWait,
        ci_check: &mut Option<CiCheck>,
    ) -> Result<(), RunError> {
        cli_eprintln_quietable!(self.log, "Waiting for remote job {job_uuid} to complete...");
        cli_eprintln_quietable!(
            self.log,
            "Note: If you interrupt (Ctrl+C), the remote job will continue running."
        );

        let FinishedJob {
            report,
            output,
            result,
        } = wait
            .until_finished(self.log, || self.get_job(project, job_uuid))
            .await?;
        self.log_job_output(output.as_ref());
        if let Err(err) = result {
            self.best_effort_display_report(project, report, ci_check)
                .await;
            return Err(err);
        }
        let report = self
            .fetch_report(project, report)
            .await
            .map_err(RunError::FetchReport)?;
        self.display_and_check_alerts(report, ci_check).await
    }

    #[cfg(feature = "plus")]
    async fn get_job(
        &self,
        project: &ProjectResourceId,
        job_uuid: JobUuid,
    ) -> Result<JsonJob, crate::BackendError> {
        self.backend
            .send_with(|client| {
                let project = project.clone();
                async move {
                    client
                        .proj_job_get()
                        .project(project)
                        .job(job_uuid)
                        .send()
                        .await
                }
            })
            .await
    }

    async fn display_and_check_alerts(
        &self,
        json_report: JsonReport,
        ci_check: &mut Option<CiCheck>,
    ) -> Result<(), RunError> {
        let alerts_count = usize::try_from(json_report.counts.alerts.total).unwrap_or(usize::MAX);
        self.display_results(json_report, ci_check).await?;
        if self.error_on_alert && alerts_count > 0 {
            Err(RunError::Alerts(alerts_count))
        } else {
            Ok(())
        }
    }

    #[cfg(feature = "plus")]
    fn log_job_output(&self, output: Option<&bencher_json::runner::JsonJobOutput>) {
        let Some(output) = output else {
            return;
        };
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

    #[cfg(feature = "plus")]
    async fn fetch_report(
        &self,
        project: &ProjectResourceId,
        report_uuid: bencher_json::ReportUuid,
    ) -> Result<JsonReport, crate::BackendError> {
        self.backend
            .send_with(|client| {
                let project = project.clone();
                async move {
                    client
                        .proj_report_get()
                        .project(project)
                        .report(report_uuid)
                        .send()
                        .await
                }
            })
            .await
    }

    #[cfg(feature = "plus")]
    async fn best_effort_display_report(
        &self,
        project: &ProjectResourceId,
        report_uuid: bencher_json::ReportUuid,
        ci_check: &mut Option<CiCheck>,
    ) {
        match self.fetch_report(project, report_uuid).await {
            Ok(report) => {
                if let Err(err) = self.display_and_check_alerts(report, ci_check).await {
                    cli_eprintln_quietable!(self.log, "Warning: failed to display report: {err}");
                }
            },
            Err(err) => {
                cli_eprintln_quietable!(self.log, "Warning: could not fetch report: {err}");
            },
        }
    }

    async fn display_results(
        &self,
        json_report: JsonReport,
        ci_check: &mut Option<CiCheck>,
    ) -> Result<(), RunError> {
        let console_url = self
            .backend
            .get_console_url()
            .await
            .map_err(RunError::ConsoleUrl)?;
        let source = self
            .ci
            .as_ref()
            .map_or_else(|| "cli".to_owned(), Ci::source);
        let sub_adapter = self.comment_sub_adapter(&json_report);
        let report_comment = ReportComment::new(console_url, json_report, sub_adapter, source);

        let report_str = match self.format {
            Format::Human => report_comment.human(),
            Format::Json => report_comment.json().map_err(RunError::SerializeReport)?,
            Format::Html => report_comment.html(false, None),
        };
        let newline_prefix = if self.log { "\n" } else { "" };
        cli_println!("{newline_prefix}{report_str}");

        if let Some(ci) = &self.ci {
            ci.run(ci_check.take(), &report_comment, self.log).await?;
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

#[cfg(test)]
mod tests {
    mod project_key {
        use clap::Parser as _;

        use super::super::Run;
        use crate::CliError;
        use crate::bencher::sub::RunError;
        use crate::parser::run::CliRun;

        const PROJECT_KEY: &str = "bencher_run_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh";

        fn parse_run(args: &[&str]) -> CliRun {
            CliRun::try_parse_from(std::iter::once("run").chain(args.iter().copied()))
                .expect("Failed to parse args")
        }

        #[test]
        fn without_project_errors() {
            let result = Run::try_from(parse_run(&["--key", PROJECT_KEY, "bencher", "mock"]));
            assert!(
                matches!(
                    result,
                    Err(CliError::Run(RunError::ProjectKeyRequiresProject))
                ),
                "{result:?}"
            );
        }

        #[test]
        fn with_project() {
            Run::try_from(parse_run(&[
                "--key",
                PROJECT_KEY,
                "--project",
                "my-project",
                "bencher",
                "mock",
            ]))
            .expect("project key with `--project` should be accepted");
        }

        #[cfg(feature = "plus")]
        #[test]
        fn with_qualified_image_project() {
            Run::try_from(parse_run(&[
                "--key",
                PROJECT_KEY,
                "--image",
                "localhost/my-project:v1",
            ]))
            .expect("project key with an image-derived project should be accepted");
        }

        #[cfg(feature = "plus")]
        #[test]
        fn with_unqualified_image_project() {
            Run::try_from(parse_run(&[
                "--key",
                PROJECT_KEY,
                "--image",
                "my-project:v1",
            ]))
            .expect("project key with an image-derived project should be accepted");
        }

        #[cfg(feature = "plus")]
        #[test]
        fn with_image_without_project_repository_errors() {
            // Multi-segment repositories (`{user}/{image}`) are not supported
            // by the Bencher registry, so no project can be derived
            let result = Run::try_from(parse_run(&[
                "--key",
                PROJECT_KEY,
                "--image",
                "ghcr.io/owner/repo:v1",
            ]));
            assert!(
                matches!(
                    result,
                    Err(CliError::Run(RunError::ProjectKeyRequiresProject))
                ),
                "{result:?}"
            );
        }
    }

    #[cfg(feature = "plus")]
    mod attach {
        use bencher_json::{JobUuid, JsonReport, PollTimeout, ProjectResourceId, Timeout};
        use clap::Parser as _;

        use super::super::{AttachJob, Job, JobWait, Run};
        use crate::parser::run::CliRun;

        const JOB: &str = "8d2b6c4e-5f3a-4b1c-9e7d-0a1b2c3d4e5f";
        const UUID: &str = "4f6d1b2a-3c5e-4d7f-8a9b-0c1d2e3f4a5b";
        const TIME: &str = "2026-01-01T00:00:00Z";

        fn parse_run(args: &[&str]) -> CliRun {
            CliRun::try_parse_from(std::iter::once("run").chain(args.iter().copied()))
                .expect("Failed to parse args")
        }

        /// A report with one iteration, holding one result per list of measure slugs.
        fn json_report(results: &[&[&str]]) -> JsonReport {
            let results = results
                .iter()
                .map(|slugs| {
                    let measures = slugs
                        .iter()
                        .map(|slug| {
                            serde_json::json!({
                                "measure": {
                                    "uuid": UUID,
                                    "project": UUID,
                                    "name": slug,
                                    "slug": slug,
                                    "units": "units",
                                    "created": TIME,
                                    "modified": TIME
                                },
                                "metrics": [],
                                "threshold": null,
                                "boundary": null
                            })
                        })
                        .collect::<Vec<_>>();
                    serde_json::json!({
                        "iteration": 0,
                        "benchmark": {
                            "uuid": UUID,
                            "project": UUID,
                            "name": "bench",
                            "slug": "bench",
                            "created": TIME,
                            "modified": TIME
                        },
                        "variant": { "uuid": UUID, "parameters": {} },
                        "measures": measures
                    })
                })
                .collect::<Vec<_>>();
            serde_json::from_value(serde_json::json!({
                "uuid": UUID,
                "project": {
                    "uuid": UUID,
                    "organization": UUID,
                    "name": "Project",
                    "slug": "project",
                    "visibility": "public",
                    "bmf_version": 0,
                    "created": TIME,
                    "modified": TIME
                },
                "branch": {
                    "uuid": UUID,
                    "project": UUID,
                    "name": "main",
                    "slug": "main",
                    "head": {
                        "uuid": UUID,
                        "version": { "number": 1 },
                        "created": TIME
                    },
                    "created": TIME,
                    "modified": TIME
                },
                "testbed": {
                    "uuid": UUID,
                    "project": UUID,
                    "name": "base",
                    "slug": "base",
                    "created": TIME,
                    "modified": TIME
                },
                "start_time": TIME,
                "end_time": TIME,
                "adapter": "magic",
                "results": [results],
                "created": TIME
            }))
            .unwrap()
        }

        #[test]
        fn attach_job() {
            let run = Run::try_from(parse_run(&[
                "--project",
                "my-project",
                "--job",
                JOB,
                "--job-timeout",
                "7",
                "--job-poll-interval",
                "3",
            ]))
            .expect("`--job` with `--project` should be accepted");
            let Some(Job::Attach(AttachJob {
                project,
                uuid,
                wait,
            })) = run.job
            else {
                panic!("{:?}", run.job);
            };
            assert_eq!(project, "my-project".parse::<ProjectResourceId>().unwrap());
            assert_eq!(uuid, JOB.parse::<JobUuid>().unwrap());
            assert_eq!(
                wait,
                JobWait::attach(
                    PollTimeout::try_from(3).unwrap(),
                    Some(Timeout::try_from(7).unwrap())
                )
            );
            // The attach neither runs a command nor reads stdin.
            assert!(run.runner.is_none());
        }

        #[test]
        fn submit_job_wait() {
            let run = Run::try_from(parse_run(&[
                "--project",
                "my-project",
                "--image",
                "alpine:3.18",
                "--job-timeout",
                "7",
                "--job-poll-interval",
                "3",
            ]))
            .expect("`--image` should be accepted");
            let Some(Job::Submit(job)) = run.job else {
                panic!("{:?}", run.job);
            };
            assert_eq!(
                job.wait,
                JobWait::submitted(
                    PollTimeout::try_from(3).unwrap(),
                    Some(Timeout::try_from(7).unwrap())
                )
            );
        }

        #[test]
        fn attach_comment_sub_adapter_follows_the_report() {
            let run = Run::try_from(parse_run(&["--project", "my-project", "--job", JOB])).unwrap();
            for (results, expected) in [
                (&[&["build-time"][..]][..], (true, false)),
                (&[&["file-size"][..]][..], (false, true)),
                (&[&["latency"][..]][..], (false, false)),
                (
                    &[&["latency"][..], &["file-size"], &["build-time"]][..],
                    (true, true),
                ),
                (&[&["build-time", "file-size"][..]][..], (true, true)),
            ] {
                let sub_adapter = run.comment_sub_adapter(&json_report(results));
                assert_eq!((sub_adapter.build_time, sub_adapter.file_size), expected);
            }
        }

        #[test]
        fn submit_comment_sub_adapter_follows_the_flags() {
            let run = Run::try_from(parse_run(&[
                "--project",
                "my-project",
                "--image",
                "alpine:3.18",
                "--build-time",
            ]))
            .unwrap();
            let sub_adapter = run.comment_sub_adapter(&json_report(&[&["file-size"]]));
            assert_eq!(
                (sub_adapter.build_time, sub_adapter.file_size),
                (true, false)
            );
        }
    }

    #[cfg(feature = "plus")]
    mod callback {
        use bencher_json::{JsonNewCallback, sanitize_json};
        use clap::Parser as _;

        use super::super::Run;
        use crate::CliError;
        use crate::bencher::sub::RunError;
        use crate::parser::run::CliRun;

        const URL: &str = "https://user-marker:pass-marker@receiver.example:8443/path-marker/hooks?query=query-marker#fragment-marker";
        const MARKERS: [&str; 7] = [
            "user-marker",
            "pass-marker",
            "path-marker",
            "query-marker",
            "fragment-marker",
            "token-marker",
            "key-marker",
        ];

        fn parse_run(args: &[&str]) -> CliRun {
            CliRun::try_parse_from(std::iter::once("run").chain(args.iter().copied()))
                .expect("Failed to parse args")
        }

        fn callback_run(args: &[&str]) -> Result<Run, CliError> {
            let args = [
                "--project",
                "my-project",
                "--image",
                "alpine:3.18",
                "--detach",
            ]
            .iter()
            .chain(args)
            .copied()
            .collect::<Vec<_>>();
            Run::try_from(parse_run(&args))
        }

        fn secret_run() -> Run {
            callback_run(&[
                "--callback-url",
                URL,
                "--callback-header",
                "Authorization: Bearer token-marker",
                "--callback-header",
                "X-Key: first-marker",
                "--callback-header",
                "x-key: key-marker",
            ])
            .unwrap()
        }

        fn assert_no_marker(printed: &str) {
            for marker in MARKERS.iter().chain(&["first-marker"]) {
                assert!(!printed.contains(marker), "{marker} in {printed}");
            }
        }

        #[test]
        fn invalid_callback_fails_before_submitting() {
            for (args, expected) in [
                (
                    &["--callback-url", "http://receiver.example/hooks"][..],
                    "callback URL must use https",
                ),
                (
                    &[
                        "--callback-url",
                        "https://receiver.example/hooks",
                        "--callback-body",
                        r#"{"job":"{{ job.id }}"}"#,
                    ][..],
                    r#"callback body placeholder at "/job" names an unknown value"#,
                ),
                (
                    &[
                        "--callback-url",
                        "https://receiver.example/hooks",
                        "--callback-header",
                        "X Token: value-marker",
                    ][..],
                    "invalid name for callback header 1",
                ),
                (
                    &[
                        "--callback-url",
                        "https://receiver.example/hooks",
                        "--callback-header",
                        "Host: receiver.example",
                    ][..],
                    "callback header host is set by the HTTP client",
                ),
            ] {
                let Err(CliError::Run(RunError::Callback(err))) = callback_run(args) else {
                    panic!("{args:?} must fail validation");
                };
                let err = err.to_string();
                assert!(err.contains(expected), "{err}");
                assert!(!err.contains("value-marker"), "{err}");
            }
        }

        #[test]
        fn malformed_callback_header_names_the_flag_not_the_value() {
            let result = callback_run(&[
                "--callback-url",
                "https://receiver.example/hooks",
                "--callback-header",
                "X-Key: key",
                "--callback-header",
                "Authorization Bearer token-marker",
            ]);
            let Err(CliError::Run(err @ RunError::CallbackHeader(2))) = result else {
                panic!("{result:?}");
            };
            let err = err.to_string();
            assert!(err.contains("`--callback-header` 2"), "{err}");
            assert_no_marker(&err);
        }

        #[test]
        fn empty_callback_header_value_is_sent_empty() {
            for header in ["X-Empty:", "X-Empty:   "] {
                let run = callback_run(&[
                    "--callback-url",
                    "https://receiver.example/hooks",
                    "--callback-header",
                    header,
                ])
                .unwrap();
                let json_new_run = run
                    .generate_remote_report(run.submit_job().unwrap(), run.callback())
                    .unwrap();
                let sent = serde_json::to_value(&json_new_run).unwrap();
                assert_eq!(
                    sent["job"]["callback"]["headers"],
                    serde_json::json!({ "x-empty": "" }),
                    "{header}"
                );
            }
        }

        #[test]
        fn callback_takes_the_headers_in_flag_order() {
            let callback = secret_run().callback().cloned().unwrap();
            let expected = JsonNewCallback::new(
                URL,
                [
                    ("authorization", "Bearer token-marker"),
                    ("x-key", "key-marker"),
                ]
                .map(|(name, value)| (name.to_owned(), value.to_owned())),
                None,
            )
            .unwrap();
            assert_eq!(callback, expected);
        }

        #[test]
        fn sent_run_carries_the_real_callback() {
            let run = secret_run();
            let json_new_run = run
                .generate_remote_report(run.submit_job().unwrap(), run.callback())
                .unwrap();
            // What the API client sends.
            let sent = serde_json::to_value(&json_new_run).unwrap();
            assert_eq!(
                sent["job"]["callback"],
                serde_json::json!({
                    "url": URL,
                    "headers": {
                        "authorization": "Bearer token-marker",
                        "x-key": "key-marker",
                    },
                }),
            );
        }

        #[test]
        fn echo_prints_the_callback_through_sanitize_json() {
            let run = secret_run();
            let callback = run.callback().unwrap();
            let json_new_run = run
                .generate_remote_report(run.submit_job().unwrap(), Some(callback))
                .unwrap();
            let echo = Run::new_report_echo(&json_new_run, Some(callback)).unwrap();
            // A debug build prints the callback in full, and a release build sanitizes it.
            for marker in MARKERS {
                assert_eq!(
                    echo.contains(marker),
                    cfg!(debug_assertions),
                    "{marker} in {echo}"
                );
            }
            let echo: serde_json::Value = serde_json::from_str(&echo).unwrap();
            assert_eq!(echo["job"]["callback"], sanitize_json(callback));
            // Only the callback differs from what is sent.
            let mut sent = serde_json::to_value(&json_new_run).unwrap();
            sent["job"]["callback"] = echo["job"]["callback"].clone();
            assert_eq!(echo, sent);
        }

        #[test]
        fn echo_prints_only_the_callback_it_is_handed() {
            let run = secret_run();
            let json_new_run = run
                .generate_remote_report(run.submit_job().unwrap(), run.callback())
                .unwrap();
            assert!(json_new_run.job.as_ref().unwrap().callback.is_some());
            let echo = Run::new_report_echo(&json_new_run, None).unwrap();
            assert_no_marker(&echo);
            let echo: serde_json::Value = serde_json::from_str(&echo).unwrap();
            assert!(echo["job"].get("callback").is_none(), "{echo}");
        }
    }
}
