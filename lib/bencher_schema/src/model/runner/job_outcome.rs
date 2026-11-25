//! A job's outcome, once its executions have theirs: the server assembles the job's output from
//! each execution's stored output, in iteration order, and processes it once, or fails the job.

use bencher_json::{
    DateTime, JobStatus,
    runner::{JsonIterationOutput, JsonJobOutput},
};
use dropshot::HttpError;
use slog::Logger;

use crate::{
    auth_conn,
    context::ApiContext,
    error::issue_error,
    model::runner::{
        JobId, QueryJob, UpdateJob,
        job_execution::{ExecutionStatus, JobExecutions, JobStep, QueryJobExecution, end_job_now},
    },
    write_conn,
};

/// Move a job with nothing in flight to its outcome, if it has one: complete it once every
/// iteration has its outcome, or fail it when it has no time left or an execution failed.
pub async fn settle_job(
    log: &Logger,
    context: &ApiContext,
    job_id: JobId,
) -> Result<(), HttpError> {
    let now = context.clock.now();
    let job_executions = JobExecutions::get(auth_conn!(context), job_id).map_err(|e| {
        issue_error(
            "Failed to read a job's executions",
            &format!("Failed to read the executions of job ({job_id})."),
            e,
        )
    })?;
    match job_executions.step(now) {
        JobStep::Wait => Ok(()),
        JobStep::Complete => complete_job(log, context, &job_executions, now).await,
        JobStep::OutOfTime(error) | JobStep::Fail(error) => {
            fail_job(log, context, &job_executions, error).await
        },
    }
}

/// Fail a job: give it its outcome, end its executions in flight, store its output with `error`,
/// and fire its callback. Its results are not processed.
async fn fail_job(
    log: &Logger,
    context: &ApiContext,
    job_executions: &JobExecutions,
    error: String,
) -> Result<(), HttpError> {
    let job = &job_executions.job;
    let ended = end_job_now(context, job.id, JobStatus::Failed)
        .await
        .map_err(|e| {
            issue_error(
                "Failed to fail a job",
                &format!("Failed to fail job ({}).", job.id),
                e,
            )
        })?;
    if !ended {
        slog::info!(log, "Job already has its outcome"; "job_id" => ?job.id);
        return Ok(());
    }
    finish_failed_job(log, context, job_executions, error).await;
    Ok(())
}

/// Finish a job the caller just failed with [`end_job`](super::job_execution::end_job): store
/// its output with `error`, and fire its callback.
pub async fn finish_failed_job(
    log: &Logger,
    context: &ApiContext,
    job_executions: &JobExecutions,
    error: String,
) {
    let job = &job_executions.job;
    slog::warn!(log, "Job failed"; "job_id" => ?job.id, "error" => &error);
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
        bencher_otel::JobStatusKind::Failed,
    ));

    let results = match failed_job_results(context, job_executions).await {
        Ok(results) => results,
        Err(e) => {
            slog::error!(log, "Failed to assemble a failed job's output"; "job_id" => ?job.id, "error" => %e);
            Vec::new()
        },
    };
    let output = JsonJobOutput {
        results,
        error: Some(error),
    };
    if let Err(e) = context
        .oci_storage()
        .job_output()
        .put(job.config.project, job.uuid, &output)
        .await
    {
        slog::error!(log, "Failed to store job output"; "job_id" => ?job.id, "error" => %e);
    }

    // Fired after the output is stored, so a receiver that reads the job finds it.
    context.callbacks.fire(log, job.id);
}

/// Complete a job whose iterations all have their outcome: store its assembled output where a
/// job's output is stored, then process it once.
async fn complete_job(
    log: &Logger,
    context: &ApiContext,
    job_executions: &JobExecutions,
    now: DateTime,
) -> Result<(), HttpError> {
    let job = &job_executions.job;
    let results = match assemble_results(context, job_executions).await {
        Ok(results) => results,
        Err(e) => {
            slog::error!(log, "Failed to assemble a job's output"; "job_id" => ?job.id, "error" => %e);
            return fail_job(log, context, job_executions, e.to_string()).await;
        },
    };
    let output = JsonJobOutput {
        results,
        error: None,
    };
    // Stored before the job is `Completed`, so a restart that finds it `Completed` finds its
    // output too.
    if let Err(e) = context
        .oci_storage()
        .job_output()
        .put(job.config.project, job.uuid, &output)
        .await
    {
        slog::error!(log, "Failed to store job output"; "job_id" => ?job.id, "error" => %e);
    }

    let updated = UpdateJob::terminate(JobStatus::Completed, now)
        .execute_if_any_status(
            write_conn!(context),
            job.id,
            &[JobStatus::Claimed, JobStatus::Running],
        )
        .map_err(|e| {
            issue_error(
                "Failed to complete a job",
                &format!("Failed to complete job ({}).", job.id),
                e,
            )
        })?;
    if updated == 0 {
        slog::debug!(log, "Job already has its outcome"; "job_id" => ?job.id);
        return Ok(());
    }
    slog::info!(log, "Job completed"; "job_id" => ?job.id, "iterations" => output.results.len());

    #[cfg(feature = "otel")]
    {
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
            bencher_otel::JobStatusKind::Completed,
        ));
        bencher_otel::ApiMeter::record(
            bencher_otel::ApiHistogram::JobCompleteDuration(job.priority),
            job.created.elapsed_secs(now),
        );
    }

    process_completed(log, context, job, output.results, now).await
}

/// Process a `Completed` job's results into its report, and move it to `Processed`, or to
/// `Failed` when processing fails, so startup recovery does not retry it forever.
async fn process_completed(
    log: &Logger,
    context: &ApiContext,
    job: &QueryJob,
    results: Vec<JsonIterationOutput>,
    now: DateTime,
) -> Result<(), HttpError> {
    let status = if let Err(e) = job.process_results(log, context, results, now).await {
        slog::error!(log, "Failed to process job results"; "job_id" => ?job.id, "error" => %e);
        JobStatus::Failed
    } else {
        JobStatus::Processed
    };
    let updated = UpdateJob::set_status(status, now)
        .execute_if_status(write_conn!(context), job.id, JobStatus::Completed)
        .map_err(|e| {
            issue_error(
                "Failed to record a job's processing",
                &format!("Failed to record the processing of job ({}).", job.id),
                e,
            )
        })?;
    if updated == 0 {
        slog::info!(log, "Job already changed state after processing"; "job_id" => ?job.id);
        return Ok(());
    }
    context.callbacks.fire(log, job.id);
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
        if status == JobStatus::Processed {
            bencher_otel::JobStatusKind::Processed
        } else {
            bencher_otel::JobStatusKind::Failed
        },
    ));
    Ok(())
}

/// Why a job's output could not be assembled.
#[derive(Debug, thiserror::Error)]
pub enum AssembleError {
    #[error("Failed to read the output of iteration {iteration}: {error}")]
    Storage {
        iteration: i32,
        error: bencher_oci_storage::OciStorageError,
    },
    #[error("The output of iteration {0} is missing")]
    Missing(i32),
}

/// The job's results to process, from each execution's stored output in iteration order.
pub async fn assemble_results(
    context: &ApiContext,
    job_executions: &JobExecutions,
) -> Result<Vec<JsonIterationOutput>, AssembleError> {
    let job = &job_executions.job;
    let mut results = Vec::with_capacity(job_executions.executions.len());
    for (execution, output) in stored_outputs(context, job_executions).await? {
        results.extend(execution_runs(job, execution, execution.status, output));
    }
    Ok(results)
}

/// A failed job's results, which are never processed: every run of each execution, as its runner
/// sent it, as a failed job's output always was.
async fn failed_job_results(
    context: &ApiContext,
    job_executions: &JobExecutions,
) -> Result<Vec<JsonIterationOutput>, AssembleError> {
    Ok(stored_outputs(context, job_executions)
        .await?
        .into_iter()
        .flat_map(|(_, output)| output.results)
        .collect())
}

/// Each execution that ran to a result, with its stored output, in iteration order.
async fn stored_outputs<'a>(
    context: &ApiContext,
    job_executions: &'a JobExecutions,
) -> Result<Vec<(&'a QueryJobExecution, JsonJobOutput)>, AssembleError> {
    let job = &job_executions.job;
    let mut outputs = Vec::with_capacity(job_executions.executions.len());
    for execution in &job_executions.executions {
        if !matches!(
            execution.status,
            ExecutionStatus::Completed | ExecutionStatus::Failed
        ) {
            continue;
        }
        let output = context
            .oci_storage()
            .job_output()
            .get_execution(job.config.project, execution.uuid)
            .await
            .map_err(|error| AssembleError::Storage {
                iteration: execution.iteration,
                error,
            })?
            .ok_or(AssembleError::Missing(execution.iteration))?;
        outputs.push((execution, output));
    }
    Ok(outputs)
}

/// The runs an execution that ended with `status` gives its job to parse and process.
///
/// Under `allow_failure`, a failed execution is 1 gap that holds only an exit code and why it
/// failed, so the next iteration keeps its number and nothing of the failed execution is processed.
pub fn execution_runs(
    job: &QueryJob,
    execution: &QueryJobExecution,
    status: ExecutionStatus,
    output: JsonJobOutput,
) -> Vec<JsonIterationOutput> {
    if status == ExecutionStatus::Failed && job.config.allow_failure.unwrap_or_default() {
        if execution.is_whole_job(job) {
            whole_job_gaps(output)
        } else {
            vec![gap(output)]
        }
    } else {
        output.results
    }
}

/// The 1 iteration of a run that failed under `allow_failure`: its non-zero exit code and standard
/// error, or exit code 1 and the error for a run with no result, with no output to process.
fn gap(output: JsonJobOutput) -> JsonIterationOutput {
    let JsonJobOutput { results, error } = output;
    let failed = results.into_iter().find(|run| run.exit_code != 0);
    let (exit_code, stderr) = match failed {
        Some(run) => (run.exit_code, run.stderr.or(error)),
        None => (1, error),
    };
    JsonIterationOutput {
        exit_code,
        stdout: None,
        stderr,
        output: None,
    }
}

/// The runs of a whole job an older server sent as 1 execution, which failed under
/// `allow_failure`: each run that exited non-zero is a gap, and a whole job that reported no
/// result at all is 1 gap.
fn whole_job_gaps(output: JsonJobOutput) -> Vec<JsonIterationOutput> {
    if output.results.is_empty() {
        return vec![gap(output)];
    }
    output
        .results
        .into_iter()
        .map(|run| {
            if run.exit_code == 0 {
                run
            } else {
                JsonIterationOutput {
                    exit_code: run.exit_code,
                    stdout: None,
                    stderr: run.stderr,
                    output: None,
                }
            }
        })
        .collect()
}
