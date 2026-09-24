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
use bencher_json::{JobUuid, JsonJob};

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
use job::{AttachJob, FinishedJob, Job, JobOutcome, JobWait, SubmitJob};
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
        if let (Some(job_uuid), Some(job)) = (json_report.job, self.submit_job()) {
            if job.detach {
                cli_eprintln_quietable!(self.log, "Remote job submitted successfully: {job_uuid}");
                return self.display_and_check_alerts(json_report, ci_check).await;
            }
            let project = ProjectResourceId::Slug(json_report.project.slug);
            return self
                .wait_for_job(&project, job_uuid, job.wait, ci_check)
                .await;
        }

        self.display_and_check_alerts(json_report, ci_check).await
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
        let err = match outcome {
            JobOutcome::Processed => {
                let report = self
                    .fetch_report(project, report)
                    .await
                    .map_err(RunError::FetchReport)?;
                return self.display_and_check_alerts(report, ci_check).await;
            },
            JobOutcome::Failed => RunError::JobFailed(error("Unknown error")),
            JobOutcome::Canceled => RunError::JobCanceled(error("Job was canceled")),
        };
        self.best_effort_display_report(project, report, ci_check)
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
}
