use std::{future::Future, pin::Pin};

#[cfg(feature = "plus")]
use bencher_client::types::JsonNewRunJob;
use bencher_client::types::{
    Adapter, JsonAverage, JsonFold, JsonNewRun, JsonReportSettings, MeasureFilter,
};
use bencher_comment::{GitHubApi, ReportComment};
use bencher_json::{
    DateTime, JsonProject, JsonReport, ProjectResourceId, ProjectSlug, ReportUuid, RunContext,
    TestbedNameId, project::report::Iteration,
};
#[cfg(feature = "plus")]
use bencher_json::{JobUuid, JsonJob, JsonNewCallback, runner::JsonNewJobGitHub, sanitize_json};
use url::Url;

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
#[cfg(feature = "plus")]
use ci::JobFailure;
use ci::{Ci, CiCheck, PostedReport, SectionReports};
pub use error::RunError;
use format::Format;
#[cfg(feature = "plus")]
use job::{
    AttachJob, CALLBACK_SKIPPED, FinishedJob, Job, JobOutcome, JobWait, SubmitJob,
    callback_skipped, client_callback, client_github, post_notice,
};
use project::resolve_project;
use runner::Runner;
use sub_adapter::SubAdapter;

use crate::bencher::SubCmd;

use super::project::report::{Thresholds, measure_filter};

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
    measures: Option<MeasureFilter>,
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
            measure,
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

        let ci: Option<Ci> = ci.try_into().map_err(RunError::Ci)?;
        // The attach continues the detached run whose callback started its workflow, and tags its
        // comment as that run would have.
        #[cfg(feature = "plus")]
        let (ci, sub_adapter) = match (ci, &job) {
            (Some(mut ci), Some(Job::Attach(_))) => {
                let sub_adapter = ci
                    .read_dispatch(!quiet)
                    .map_or(sub_adapter, SubAdapter::from);
                (Some(ci), sub_adapter)
            },
            (ci, _) => (ci, sub_adapter),
        };

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
            measures: measure_filter(measure),
            backdate,
            allow_failure,
            thresholds: thresholds.try_into().map_err(RunError::Thresholds)?,
            error_on_alert,
            format: format.into(),
            log: !quiet,
            ci,
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
        // The attach takes over the check its detached run started before anything can fail,
        // so that an error from here on fails that check too.
        #[cfg(feature = "plus")]
        let mut ci_check = self.ci.as_ref().and_then(Ci::adopted_check);
        #[cfg(not(feature = "plus"))]
        let mut ci_check = None;

        let result = self.check_and_run(&mut ci_check).await;
        // Every path that posts results consumes the handle,
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

    async fn check_and_run(&self, ci_check: &mut Option<CiCheck>) -> Result<(), RunError> {
        let mismatch = self
            .backend
            .check_version()
            .await
            .map_err(RunError::ApiVersion)?;
        if let Some(mismatch) = &mismatch {
            cli_eprintln_quietable!(self.log, "Warning: {mismatch}");
        }
        #[cfg(feature = "plus")]
        self.check_server_posts(
            mismatch
                .as_ref()
                .map_or(crate::CLI_VERSION, |mismatch| mismatch.api_version.as_str()),
        )?;
        #[cfg(feature = "plus")]
        let server_posts = self.server_posts();
        #[cfg(not(feature = "plus"))]
        let server_posts = false;

        if let Some(ci) = &self.ci {
            ci.safety_check(self.log)?;
        }

        // Start the in-progress CI check before the benchmark runs,
        // so a rerun immediately clears the stale conclusion left by a previous run.
        // Dry runs never post results, so they never start a check, and neither does a run whose
        // check the server posts.
        if let Some(ci) = &self.ci
            && ci_check.is_none()
            && !self.dry_run
            && !server_posts
        {
            let project = if ci.needs_project() {
                self.project().await
            } else {
                None
            };
            *ci_check = ci
                .start(project.as_ref(), &self.sub_adapter.into(), self.log)
                .await;
        }

        self.run_and_report(ci_check).await
    }

    /// The server posts a run's check and comment with its GitHub App, from the release that
    /// posts, and refuses a run on any GitHub but its own.
    #[cfg(feature = "plus")]
    fn check_server_posts(&self, server_version: &str) -> Result<(), RunError> {
        if !self.server_posts() {
            Ok(())
        } else if !posts_detached_runs(server_version) {
            Err(RunError::DetachNeedsPostingServer(
                server_version.to_owned(),
            ))
        } else {
            Ok(())
        }
    }

    /// A detached run on GitHub Actions gets its check and comment from the server.
    #[cfg(feature = "plus")]
    fn server_posts(&self) -> bool {
        self.ci.is_some() && self.submit_job().is_some_and(|job| job.detach)
    }

    /// Best-effort: look up the Project, which the CI check names.
    /// The Project is only knowable before the benchmarks run if `--project` is set,
    /// so the check of an on-the-fly Project goes without it until the run completes.
    async fn project(&self) -> Option<JsonProject> {
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
                    "Could not resolve the Project for the CI check: {err}"
                );
            })
            .ok()?;
        Some(json_project)
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

        #[cfg(feature = "plus")]
        let callback = self.callback();
        #[cfg(feature = "plus")]
        let github = self.job_github()?;

        let Some(json_new_run) = self.generate_report().await? else {
            return Ok(());
        };
        #[cfg(feature = "plus")]
        let mut json_new_run = json_new_run;
        #[cfg(feature = "plus")]
        if let (Some(job), Some(github)) = (&mut json_new_run.job, &github) {
            job.github = Some(client_github(github)?);
        }

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

        #[cfg(feature = "plus")]
        if let (Some(job), Some(callback)) = (&mut json_new_run.job, callback) {
            job.callback = Some(client_callback(callback)?);
        }
        let sender = run_sender(json_new_run);
        let json_report: JsonReport = self
            .backend
            .send_with(sender)
            .await
            .map_err(RunError::SendReport)?;

        #[cfg(feature = "plus")]
        if let (Some(job_uuid), Some(job)) = (json_report.job, self.submit_job()) {
            if job.detach {
                let reporter = callback
                    .map(|_| Reporter::Callback)
                    .or(github.map(|_| Reporter::Server));
                return self.detached(json_report, job_uuid, reporter).await;
            }
            let project = ProjectResourceId::Slug(json_report.project.slug);
            return self
                .wait_for_job(&project, job_uuid, job.wait, ci_check)
                .await;
        }

        self.display_and_check_alerts(json_report, ci_check).await
    }

    /// A detached run posts nothing: it reads the job once to say how its results will be reported.
    #[cfg(feature = "plus")]
    async fn detached(
        &self,
        json_report: JsonReport,
        job_uuid: JobUuid,
        reporter: Option<Reporter>,
    ) -> Result<(), RunError> {
        cli_eprintln_quietable!(self.log, "Remote job submitted successfully: {job_uuid}");
        match reporter {
            Some(Reporter::Callback) => self.callback_notice(&json_report, job_uuid).await,
            Some(Reporter::Server) => self.print_post_state(&json_report, job_uuid).await,
            None => {},
        }
        let console_url = self.console_url().await?;
        self.display_report(console_url, json_report, true)
            .map(drop)
    }

    async fn generate_report(&self) -> Result<Option<JsonNewRun>, RunError> {
        #[cfg(feature = "plus")]
        if let Some(job) = self.submit_job() {
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
                measures: self.measures.clone(),
            }),
            context: Some(RunContext::current().into()),
            job: None,
        }))
    }

    #[cfg(feature = "plus")]
    fn generate_remote_report(&self, job: &SubmitJob) -> JsonNewRun {
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
                measures: self.measures.clone(),
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
                callback: None,
                github: None,
            }),
        }
    }

    /// The new report as printed, with the callback, which can hold secrets, through `sanitize_json`.
    fn new_report_echo(
        json_new_run: &JsonNewRun,
        #[cfg(feature = "plus")] callback: Option<&JsonNewCallback>,
    ) -> Result<String, RunError> {
        #[cfg(feature = "plus")]
        if let Some(callback) = callback {
            let mut echo = serde_json::to_value(json_new_run).map_err(RunError::SerializeReport)?;
            if let Some(job) = echo
                .get_mut("job")
                .and_then(serde_json::Value::as_object_mut)
            {
                job.insert("callback".to_owned(), sanitize_json(callback));
            }
            return serde_json::to_string_pretty(&echo).map_err(RunError::SerializeReport);
        }
        serde_json::to_string_pretty(json_new_run).map_err(RunError::SerializeReport)
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

    /// The pull request context the server posts a detached run's check and comment for.
    #[cfg(feature = "plus")]
    fn job_github(&self) -> Result<Option<JsonNewJobGitHub>, RunError> {
        if let Some(ci) = &self.ci
            && self.server_posts()
        {
            ci.job_github(self.log).map_err(RunError::Ci)
        } else {
            Ok(None)
        }
    }

    /// Read the detached job once, and say so if the server skipped its callback.
    /// The job was submitted either way, so a failed read prints nothing.
    #[cfg(feature = "plus")]
    async fn callback_notice(&self, json_report: &JsonReport, job_uuid: JobUuid) {
        let project = ProjectResourceId::Slug(json_report.project.slug.clone());
        if let Ok(json_job) = self.get_job(&project, job_uuid).await
            && callback_skipped(&json_job)
        {
            cli_eprintln_quietable!(self.log, "{CALLBACK_SKIPPED}");
        }
    }

    /// Read the detached job once, and say how the server's post of its check and comment stands.
    /// The job runs either way, so a failed read prints nothing.
    #[cfg(feature = "plus")]
    async fn print_post_state(&self, json_report: &JsonReport, job_uuid: JobUuid) {
        let project = ProjectResourceId::Slug(json_report.project.slug.clone());
        if let Ok(json_job) = self.get_job(&project, job_uuid).await
            && let Some(notice) = post_notice(&json_job)
        {
            cli_eprintln_quietable!(self.log, "{notice}");
        }
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
            outcome,
            output,
        } = wait
            .until_finished(self.log, || self.get_job(project, job_uuid))
            .await?;
        self.log_job_output(output.as_ref());
        let error = |default: &str| {
            output
                .and_then(|output| output.error)
                .unwrap_or_else(|| default.to_owned())
        };
        let (err, failure) = match outcome {
            JobOutcome::Processed => {
                let report = self
                    .fetch_report(project, report)
                    .await
                    .map_err(RunError::FetchReport)?;
                return self.display_and_check_alerts(report, ci_check).await;
            },
            JobOutcome::Failed => (
                RunError::JobFailed(error("Unknown error")),
                JobFailure::Failed,
            ),
            JobOutcome::Canceled => (
                RunError::JobCanceled(error("Job was canceled")),
                JobFailure::Canceled,
            ),
        };
        self.best_effort_display_report(project, report, failure, ci_check)
            .await;
        Err(err)
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
        self.post_results(
            json_report,
            #[cfg(feature = "plus")]
            None,
            ci_check,
        )
        .await
    }

    /// Display the report and post it to CI, with the failure of a job that ended without results.
    async fn post_results(
        &self,
        json_report: JsonReport,
        #[cfg(feature = "plus")] failure: Option<JobFailure>,
        ci_check: &mut Option<CiCheck>,
    ) -> Result<(), RunError> {
        let alerts_count = usize::try_from(json_report.counts.alerts.total).unwrap_or(usize::MAX);
        let (project, report) = (json_report.project.uuid, json_report.uuid);
        let console_url = self.console_url().await?;
        let report_comment = self.display_report(console_url.clone(), json_report, false)?;
        if let Some(ci) = &self.ci {
            let report = PostedReport {
                comment: &report_comment,
                project,
                report,
                sub_adapter: self.sub_adapter.into(),
                console_url,
            };
            ci.run(
                ci_check.take(),
                &report,
                self,
                #[cfg(feature = "plus")]
                failure,
                self.log,
            )
            .await?;
        }
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
        report_uuid: ReportUuid,
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
        report_uuid: ReportUuid,
        failure: JobFailure,
        ci_check: &mut Option<CiCheck>,
    ) {
        match self.fetch_report(project, report_uuid).await {
            Ok(report) => {
                if let Err(err) = self.post_results(report, Some(failure), ci_check).await {
                    cli_eprintln_quietable!(self.log, "Warning: failed to display report: {err}");
                }
            },
            Err(err) => {
                cli_eprintln_quietable!(self.log, "Warning: could not fetch report: {err}");
            },
        }
    }

    async fn console_url(&self) -> Result<Url, RunError> {
        self.backend
            .get_console_url()
            .await
            .map_err(RunError::ConsoleUrl)
    }

    fn display_report(
        &self,
        console_url: Url,
        json_report: JsonReport,
        pending: bool,
    ) -> Result<ReportComment, RunError> {
        let report_comment = self.report_comment(
            console_url,
            json_report,
            pending,
            ci::github_api_url(false).as_deref(),
        );

        let report_str = match self.format {
            Format::Human => report_comment.human(),
            Format::Json => report_comment.json().map_err(RunError::SerializeReport)?,
            Format::Html => report_comment.html(false, None),
        };
        let newline_prefix = if self.log { "\n" } else { "" };
        cli_println!("{newline_prefix}{report_str}");
        Ok(report_comment)
    }

    fn report_comment(
        &self,
        console_url: Url,
        json_report: JsonReport,
        pending: bool,
        github_api_url: Option<&str>,
    ) -> ReportComment {
        let github_api = self
            .ci
            .as_ref()
            .map_or(GitHubApi::Other, |ci| ci.github_api(github_api_url));
        ReportComment::new(
            console_url,
            json_report,
            self.sub_adapter.into(),
            self.source(),
        )
        .with_pending(pending)
        .with_github_api(github_api)
    }

    fn source(&self) -> String {
        self.ci
            .as_ref()
            .map_or_else(|| "cli".to_owned(), Ci::source)
    }
}

impl SectionReports for Run {
    async fn section_report(
        &self,
        console_url: &Url,
        project: &ProjectSlug,
        report: ReportUuid,
        sub_adapter: bencher_comment::SubAdapter,
    ) -> Option<ReportComment> {
        let json_report: JsonReport = self
            .backend
            .send_once(|client| {
                let project = ProjectResourceId::Slug(project.clone());
                async move {
                    client
                        .proj_report_get()
                        .project(project)
                        .report(report)
                        .send()
                        .await
                }
            })
            .await
            .ok()?;
        Some(ReportComment::new(
            console_url.clone(),
            json_report,
            sub_adapter,
            self.source(),
        ))
    }
}

/// Who reports a detached job's results once it finishes.
#[cfg(feature = "plus")]
#[derive(Debug, Clone, Copy)]
enum Reporter {
    Callback,
    /// The server, which posts the check and comment.
    Server,
}

/// The first server version that posts the check and comment of a detached run, and refuses one
/// on a GitHub that is not its own.
#[cfg(feature = "plus")]
const POSTING_VERSION: semver::Version = semver::Version::new(0, 6, 18);

#[cfg(feature = "plus")]
fn posts_detached_runs(server_version: &str) -> bool {
    semver::Version::parse(server_version).is_ok_and(|version| version >= POSTING_VERSION)
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
        use bencher_json::{JobUuid, PollTimeout, ProjectResourceId, Timeout};
        use clap::Parser as _;

        use super::super::{AttachJob, Job, JobWait, Run};
        use crate::parser::run::CliRun;

        const JOB: &str = "8d2b6c4e-5f3a-4b1c-9e7d-0a1b2c3d4e5f";

        fn parse_run(args: &[&str]) -> CliRun {
            CliRun::try_parse_from(std::iter::once("run").chain(args.iter().copied()))
                .expect("Failed to parse args")
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
    }

    mod github_link {
        use bencher_json::JsonReport;
        use clap::Parser as _;

        use super::super::Run;
        use crate::parser::run::CliRun;

        const LINE: &str = "Link this project to GitHub</a>";

        fn run(args: &[&str]) -> Run {
            let args = ["--project", "project"]
                .iter()
                .chain(args)
                .chain(&["bencher", "mock"])
                .copied()
                .collect::<Vec<_>>();
            Run::try_from(CliRun::try_parse_from(std::iter::once("run").chain(args)).unwrap())
                .unwrap()
        }

        /// A report of a claimed project with no linked repository.
        fn unlinked_report() -> JsonReport {
            const UUID: &str = "4f6d1b2a-3c5e-4d7f-8a9b-0c1d2e3f4a5b";
            const TIME: &str = "2026-01-01T00:00:00Z";
            serde_json::from_value(serde_json::json!({
                "uuid": UUID,
                "project": {
                    "uuid": UUID,
                    "organization": UUID,
                    "name": "Project",
                    "slug": "project",
                    "github_link": "unlinked",
                    "visibility": "public",
                    "bmf_version": 0,
                    "created": TIME,
                    "modified": TIME,
                    "claimed": TIME
                },
                "branch": {
                    "uuid": UUID,
                    "project": UUID,
                    "name": "main",
                    "slug": "main",
                    "head": { "uuid": UUID, "version": { "number": 1 }, "created": TIME },
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
                "results": [],
                "created": TIME
            }))
            .unwrap()
        }

        // Kills a comment that asks to link on GitHub Enterprise Server or on any API but
        // github.com's, a prefix match on github.com's API URL, a comment that asks outside of a
        // CI, and a run whose comment never learns its GitHub API.
        #[test]
        fn only_github_com_s_api_asks_to_link_in_the_comment() {
            let github_actions = run(&["--github-actions", "token"]);
            let no_ci = run(&[]);
            for (run, api_url, asks) in [
                (&github_actions, None, true),
                (&github_actions, Some("https://api.github.com"), true),
                (&github_actions, Some("https://api.github.com/"), true),
                (
                    &github_actions,
                    Some("https://github.example.com/api/v3"),
                    false,
                ),
                (
                    &github_actions,
                    Some("https://api.github.com/api/v3"),
                    false,
                ),
                (&github_actions, Some("http://api.github.com"), false),
                (&no_ci, None, false),
            ] {
                let comment = run.report_comment(
                    "http://localhost:3000".parse().unwrap(),
                    unlinked_report(),
                    false,
                    api_url,
                );
                let html = comment.html(false, None);
                assert_eq!(html.contains(LINE), asks, "{api_url:?}: {html}");
                assert!(
                    comment.human().contains("Link this project to GitHub: "),
                    "{api_url:?}"
                );
            }
        }
    }

    #[cfg(feature = "plus")]
    mod server_posts {
        use clap::Parser as _;

        use super::super::Run;
        use crate::bencher::sub::RunError;
        use crate::parser::run::CliRun;

        fn run(args: &[&str]) -> Run {
            let args = ["--project", "my-project", "--image", "alpine:3.18"]
                .iter()
                .chain(args)
                .copied()
                .collect::<Vec<_>>();
            Run::try_from(CliRun::try_parse_from(std::iter::once("run").chain(args)).unwrap())
                .unwrap()
        }

        // Kills a refusal before the server's version is known, a gate on any version but the
        // first that posts, a gate on a run that waits, and a refusal for the wrong cause.
        #[test]
        fn a_detached_run_needs_a_server_that_posts() {
            let detached = run(&["--detach", "--github-actions", "token"]);
            let waiting = run(&["--github-actions", "token"]);
            for (run, version, refused) in [
                (&detached, "0.6.17", true),
                (&detached, "0.6.18-rc.1", true),
                (&detached, "not-a-version", true),
                (&detached, "0.6.18", false),
                (&detached, "0.6.19", false),
                (&detached, "0.7.0", false),
                (&waiting, "0.6.15", false),
            ] {
                let checked = run.check_server_posts(version);
                match &checked {
                    Ok(()) => assert!(!refused, "{version}"),
                    Err(RunError::DetachNeedsPostingServer(server)) => {
                        assert!(refused, "{version}");
                        assert_eq!(server, version);
                    },
                    Err(err) => panic!("{version}: {err}"),
                }
            }
        }
    }

    #[cfg(feature = "plus")]
    mod callback {
        use bencher_json::JsonNewCallback;
        use clap::Parser as _;

        use super::super::{Run, client_callback};
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
                    r#"callback body placeholder "{{ job.id }}" names an unknown value"#,
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
                let sent = client_callback(run.callback().unwrap()).unwrap();
                assert_eq!(
                    serde_json::to_value(&sent).unwrap()["headers"],
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
    }

    mod measures {
        use clap::Parser as _;

        use super::super::Run;
        use crate::parser::run::CliRun;

        /// Kills: a `--measure` list that the parse discards, and a local run whose request
        /// sends no list.
        #[tokio::test]
        async fn local_run_sends_the_listed_measures() {
            assert_eq!(
                sent_measures(&["--file", "Cargo.toml"]).await,
                serde_json::json!(["latency", "Wall Clock"])
            );
        }

        /// Kills: a `--measure` list that the parse discards, and a remote run whose request
        /// sends no list.
        #[cfg(feature = "plus")]
        #[tokio::test]
        async fn image_run_sends_the_listed_measures() {
            assert_eq!(
                sent_measures(&["--image", "alpine:3.18"]).await,
                serde_json::json!(["latency", "Wall Clock"])
            );
        }

        /// The measures the request of a run with `args` lists to keep.
        async fn sent_measures(args: &[&str]) -> serde_json::Value {
            let args = [
                "run",
                "--project",
                "my-project",
                "--measure",
                "latency",
                "--measure",
                "Wall Clock",
            ]
            .iter()
            .chain(args)
            .copied();
            let run = Run::try_from(CliRun::try_parse_from(args).unwrap()).unwrap();
            let json_new_run = run.generate_report().await.unwrap().unwrap();
            serde_json::to_value(json_new_run).unwrap()["settings"]["measures"].clone()
        }
    }
}
