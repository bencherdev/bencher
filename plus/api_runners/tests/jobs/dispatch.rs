//! The server sends a job's iterations one execution at a time: each `Job` message carries 1 run
//! of the job's image under its own UUID, and the server forms the iterations, orders the jobs,
//! and owns the timeout, the failures, and the bill.

use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

use api_runners::{RunnerMessage, ServerMessage};
use bencher_api_tests::{TestServer, TestServerOptions, TestUser};
use bencher_config::spawn_job_recovery;
use bencher_json::{
    DateTime, JobStatus, JobUuid, JsonClaimedJob, PollTimeout, Priority, RunnerUuid,
    project::report::{Adapter, ParseOutcome},
    runner::{JsonIterationOutput, JsonReady},
};
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use futures::StreamExt as _;
use tokio_tungstenite::tungstenite::Message;

use super::{
    common::{
        TEST_SOURCE_IP, WsStream, assert_one_callback, associate_runner_spec, connect_channel_ws,
        create_runner, create_sibling_report, create_test_report, get_project_id, get_runner_id,
        insert_pending_callback, insert_test_spec, recv_server_msg, runner_metadata,
        send_runner_msg,
    },
    get_status,
};

/// A job of 3 iterations goes out as 3 `Job` messages, each of 1 iteration under its own UUID,
/// and its report stores the 3 runs as iterations 0, 1, and 2, in order.
///
/// Kills: a server that sends the whole job in 1 message, reuses the job's UUID, or numbers the
/// iterations in the order the results arrive rather than the order they were sent.
#[tokio::test]
async fn a_job_of_3_iterations_is_3_messages() {
    let lab = Lab::new("three").await;
    let job = lab.job(Job::plus().iterations(3));
    let mut runner = lab.runner("Runner").await;

    let mut uuids = Vec::new();
    for value in [1.0, 2.0, 3.0] {
        let execution = runner.claim().await;
        assert_eq!(execution.config.iter.map(u32::from), Some(1), "1 iteration");
        uuids.push(execution.uuid);
        runner.run(execution.uuid, value).await;
    }

    assert_ne!(uuids[0], uuids[1]);
    assert_ne!(uuids[1], uuids[2]);
    assert_ne!(uuids[0], uuids[2]);
    assert!(!uuids.contains(&job), "an execution has its own UUID");
    assert_eq!(get_status(&lab.server, job), JobStatus::Processed);
    assert_eq!(
        lab.stored(job),
        vec![(0, 1.0), (1, 2.0), (2, 3.0)],
        "each run is the iteration it was sent as"
    );
    assert!(runner.poll().await.is_none(), "nothing more to send");
}

/// A job of 0 iterations is sent once, as a job of 1, so it is never left pending.
#[tokio::test]
async fn a_job_of_0_iterations_runs_once() {
    let lab = Lab::new("zero").await;
    let job = lab.job(Job::plus().iterations(0));
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    runner.run(execution.uuid, 1.0).await;

    assert_eq!(get_status(&lab.server, job), JobStatus::Processed);
    assert!(runner.poll().await.is_none(), "nothing more to send");
}

/// A result names its execution by the execution's UUID, so a result resent after the next
/// execution went out lands on its own iteration, and the runner that ran it is the one checked.
///
/// Kills: a server that pins a result on the job's latest execution, or drops a result whose job
/// has since gone to another runner.
#[tokio::test]
async fn a_resent_result_lands_on_its_own_execution() {
    let lab = Lab::without_heartbeat_timeout("resend").await;
    let job = lab.job(Job::plus().iterations(2));
    let mut first = lab.runner("First").await;
    let mut second = lab.runner("Second").await;

    let execution_0 = first.claim().await;
    first.running().await;
    first.disconnect().await;

    let execution_1 = second.claim().await;
    second.running().await;
    assert_eq!(
        lab.job_runner(job),
        Some(second.id),
        "the latest execution's"
    );

    // The other runner cannot finish an execution that is not its own.
    let mut second_again = second.reconnect(&lab).await;
    second_again
        .resend(RunnerMessage::Completed {
            job: execution_0.uuid,
            results: vec![bmf(9.0)],
        })
        .await;
    assert_eq!(
        lab.execution_status(execution_0.uuid),
        Some(ExecutionState::RUNNING)
    );

    let mut first_again = first.reconnect(&lab).await;
    first_again
        .resend(RunnerMessage::Completed {
            job: execution_0.uuid,
            results: vec![bmf(1.0)],
        })
        .await;
    second.complete(execution_1.uuid, 2.0).await;

    assert_eq!(get_status(&lab.server, job), JobStatus::Processed);
    assert_eq!(lab.stored(job), vec![(0, 1.0), (1, 2.0)]);
}

/// Between jobs of one tier, the job whose last execution went out longest ago goes next, and a
/// job with none goes first: a job of 1 iteration waits for at most 1 iteration of a longer job.
///
/// Kills: a claim that keeps going to the job it last sent, and an order by creation alone.
#[tokio::test]
async fn a_short_job_runs_after_the_first_iteration_of_a_long_one() {
    let lab = Lab::new("short").await;
    let long = lab.job(Job::plus().iterations(3));
    let short = lab.job(Job::plus().iterations(1));
    let mut runner = lab.runner("Runner").await;

    assert_eq!(
        runner.run_all(&lab).await,
        vec![long, short, long, long],
        "the short job waits for 1 iteration of the long one"
    );
}

/// 2 jobs of 2 iterations each in one tier alternate.
#[tokio::test]
async fn two_jobs_alternate() {
    let lab = Lab::new("alternate").await;
    let a = lab.job(Job::plus().iterations(2));
    let b = lab.job(Job::plus().iterations(2));
    let mut runner = lab.runner("Runner").await;

    assert_eq!(runner.run_all(&lab).await, vec![a, b, a, b]);
}

/// The tier still comes first: a Plus job runs all of its iterations before an older Free job.
///
/// Kills: an order that puts the oldest last execution ahead of the tier.
#[tokio::test]
async fn a_plus_job_goes_before_a_free_job() {
    let lab = Lab::new("tier").await;
    let free = lab.job(Job::free().iterations(2));
    let plus = lab.job(Job::plus().iterations(2));
    let mut runner = lab.runner("Runner").await;

    assert_eq!(runner.run_all(&lab).await, vec![plus, plus, free, free]);
}

/// 2 runners take iterations of 1 job at the same time, and each execution records its runner.
///
/// Kills: a claim that skips a job with an execution in flight, and a record of the job's runner
/// only.
#[tokio::test]
async fn two_runners_run_one_job_at_once() {
    let lab = Lab::without_heartbeat_timeout("two-runners").await;
    let job = lab.job(Job::plus().iterations(2));
    let mut first = lab.runner("First").await;
    let mut second = lab.runner("Second").await;

    let execution_0 = first.claim().await;
    first.running().await;
    let execution_1 = second.claim().await;
    second.running().await;

    assert_eq!(
        lab.executions(job),
        vec![
            (0, first.id, ExecutionState::RUNNING),
            (1, second.id, ExecutionState::RUNNING),
        ]
    );

    second.complete(execution_1.uuid, 2.0).await;
    first.complete(execution_0.uuid, 1.0).await;
    assert_eq!(get_status(&lab.server, job), JobStatus::Processed);
    assert_eq!(lab.stored(job), vec![(0, 1.0), (1, 2.0)]);
}

/// A Free organization's job between iterations is still in progress, so its next job waits.
///
/// Kills: a tier check that counts only a job with an execution in flight, and one that blocks a
/// job on itself.
#[tokio::test]
async fn a_free_job_between_iterations_holds_its_organization() {
    let lab = Lab::new("free-between").await;
    let first = lab.job(Job::free().iterations(2));
    let second = lab.job(Job::free().iterations(1));
    let mut runner = lab.runner("Runner").await;

    assert_eq!(runner.run_all(&lab).await, vec![first, first, second]);
}

/// When 2 jobs of a Free organization are both in progress, the one sent first goes on, and the
/// other waits for it: a job lost to a silent runner frees its organization, so another can start,
/// and then both are in progress once its result lands.
///
/// Kills: a tier check where each job in progress blocks the other, so neither is sent again, and
/// one where the job sent later goes on. The job sent first has the larger id, so only the time
/// each was sent decides.
#[tokio::test]
async fn two_free_jobs_in_progress_take_turns() {
    let lab = Lab::new("free-both").await;
    let second = lab.job(Job::free().iterations(2));
    let first = lab.job(Job::free().iterations(2));
    lab.make_oldest(first);
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    assert_eq!(lab.execution_job(execution.uuid), first);
    runner.running().await;
    go_silent().await;
    lab.advance(10);
    let mut runner = runner.reconnect(&lab).await;
    let other = runner.claim().await;
    assert_eq!(
        lab.execution_job(other.uuid),
        second,
        "a lost job frees its organization"
    );
    runner.run(other.uuid, 2.0).await;
    runner
        .resend(RunnerMessage::Completed {
            job: execution.uuid,
            results: vec![bmf(1.0)],
        })
        .await;
    assert_eq!(get_status(&lab.server, first), JobStatus::Running);
    assert_eq!(get_status(&lab.server, second), JobStatus::Running);

    assert_eq!(runner.run_all(&lab).await, vec![first, second]);
    assert_eq!(get_status(&lab.server, first), JobStatus::Processed);
    assert_eq!(get_status(&lab.server, second), JobStatus::Processed);
}

/// When 2 jobs of a Free organization in progress were sent at the same time, the one with the
/// smaller id goes on, and the other waits for it.
///
/// Kills: a tie-break where the larger id goes on.
#[tokio::test]
async fn two_free_jobs_sent_at_once_take_turns_by_id() {
    let lab = Lab::new("free-tie").await;
    let first = lab.job(Job::free().iterations(2));
    let second = lab.job(Job::free().iterations(2));
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    assert_eq!(lab.execution_job(execution.uuid), first);
    runner.running().await;
    go_silent().await;
    let mut runner = runner.reconnect(&lab).await;
    let other = runner.claim().await;
    assert_eq!(lab.execution_job(other.uuid), second);
    runner.run(other.uuid, 2.0).await;
    runner
        .resend(RunnerMessage::Completed {
            job: execution.uuid,
            results: vec![bmf(1.0)],
        })
        .await;

    assert_eq!(runner.run_all(&lab).await, vec![first, second]);
}

/// Without `allow_failure`, a failed second execution fails the job, and the third is never sent.
#[tokio::test]
async fn a_failed_execution_fails_the_job() {
    let lab = Lab::new("fail").await;
    let job = lab.job(Job::plus().iterations(3));
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    runner.run(execution.uuid, 1.0).await;
    let execution = runner.claim().await;
    runner.running().await;
    let ack = runner
        .send(RunnerMessage::Failed {
            job: execution.uuid,
            results: vec![exit(1)],
            error: "Benchmark exited with non-zero exit code: 1".to_owned(),
        })
        .await;
    assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");

    assert_eq!(get_status(&lab.server, job), JobStatus::Failed);
    assert!(runner.poll().await.is_none(), "no third execution");
    assert_eq!(lab.stored(job), Vec::new(), "a failed job is not processed");
    let output = lab.output(job).await.expect("the job's output is stored");
    assert_eq!(output.results.len(), 2, "both runs' outputs");
    assert!(output.error.is_some());
}

/// With `allow_failure`, a failed second execution leaves iteration 1 a gap, and the third runs.
///
/// Kills: a server that ingests the failed run, as before, or renumbers the third run as 1.
#[tokio::test]
async fn an_allowed_failure_is_a_gap() {
    let lab = Lab::new("allow").await;
    let job = lab.job(Job::plus().iterations(3).allow_failure());
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    runner.run(execution.uuid, 1.0).await;
    let execution = runner.claim().await;
    runner.running().await;
    // A runner with `allow_failure` reports a non-zero exit as completed.
    let mut failed = bmf(2.0);
    failed.exit_code = 1;
    let ack = runner
        .send(RunnerMessage::Completed {
            job: execution.uuid,
            results: vec![failed],
        })
        .await;
    assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");
    let execution = runner.claim().await;
    runner.run(execution.uuid, 3.0).await;

    assert_eq!(get_status(&lab.server, job), JobStatus::Processed);
    assert_eq!(lab.stored(job), vec![(0, 1.0), (2, 3.0)]);
}

/// With `allow_failure`, a run with no result is a gap too, and the job goes on.
///
/// Kills: a server that fails the job on a run the runner reported with no result.
#[tokio::test]
async fn a_run_with_no_result_is_a_gap_under_allow_failure() {
    let lab = Lab::new("allow-empty").await;
    let job = lab.job(Job::plus().iterations(3).allow_failure());
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    runner.run(execution.uuid, 1.0).await;
    let execution = runner.claim().await;
    runner.running().await;
    // A runner with `allow_failure` reports a run that ended in an error as completed with no
    // result.
    let ack = runner
        .send(RunnerMessage::Completed {
            job: execution.uuid,
            results: Vec::new(),
        })
        .await;
    assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");
    let execution = runner.claim().await;
    runner.run(execution.uuid, 3.0).await;

    assert_eq!(get_status(&lab.server, job), JobStatus::Processed);
    assert_eq!(lab.stored(job), vec![(0, 1.0), (2, 3.0)]);
}

/// With `allow_failure`, an image that cannot be pulled fails the job at its first execution.
///
/// Kills: a server that reads a `Failed` under `allow_failure` as a gap, so it sends every
/// execution and processes a job with nothing to store.
#[tokio::test]
async fn a_failed_pull_fails_the_job_under_allow_failure() {
    let lab = Lab::new("allow-pull").await;
    let job = lab.job(Job::plus().iterations(3).allow_failure());
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    runner.running().await;
    let error = "Failed to pull image: manifest unknown".to_owned();
    let ack = runner
        .send(RunnerMessage::Failed {
            job: execution.uuid,
            results: Vec::new(),
            error: error.clone(),
        })
        .await;
    assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");

    assert_eq!(get_status(&lab.server, job), JobStatus::Failed);
    assert!(runner.poll().await.is_none(), "no second execution");
    assert_eq!(lab.stored(job), Vec::new(), "a failed job is not processed");
    let output = lab.output(job).await.expect("the job's output is stored");
    assert_eq!(output.error, Some(error));
}

/// With `allow_failure`, a run the runner stops when its budget runs out fails the job with the
/// runner's error.
///
/// Kills: a server that reads the `Failed` as a gap and fails the job only because no time is left.
#[tokio::test]
async fn a_failed_timeout_fails_the_job_under_allow_failure() {
    let lab = Lab::new("allow-timeout").await;
    let job = lab.job(Job::plus().iterations(3).timeout(100).allow_failure());
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    runner.running().await;
    lab.advance(100);
    let error = "Benchmark timed out after 100s".to_owned();
    let ack = runner
        .send(RunnerMessage::Failed {
            job: execution.uuid,
            results: Vec::new(),
            error: error.clone(),
        })
        .await;
    assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");

    assert_eq!(get_status(&lab.server, job), JobStatus::Failed);
    assert!(runner.poll().await.is_none(), "no second execution");
    let output = lab.output(job).await.expect("the job's output is stored");
    assert_eq!(output.error, Some(error));
}

/// With `allow_failure`, an execution that answers `Failed` fails the job, even when its run
/// exited 0 with valid output, and the job's stored output keeps that run as the runner sent it.
///
/// Kills: a gap for a `Failed`, and a failed job's output that reduces its failed execution's run
/// to a gap.
#[tokio::test]
async fn a_failed_clean_run_fails_the_job_under_allow_failure() {
    let lab = Lab::new("allow-clean").await;
    let job = lab.job(Job::plus().iterations(3).allow_failure());
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    runner.run(execution.uuid, 1.0).await;
    let execution = runner.claim().await;
    runner.running().await;
    let ack = runner
        .send(RunnerMessage::Failed {
            job: execution.uuid,
            results: vec![bmf(2.0)],
            error: "the runner stopped".to_owned(),
        })
        .await;
    assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");

    assert_eq!(get_status(&lab.server, job), JobStatus::Failed);
    assert!(runner.poll().await.is_none(), "no third execution");
    assert_eq!(lab.stored(job), Vec::new(), "a failed job is not processed");
    let output = lab.output(job).await.expect("the job's output is stored");
    let stdouts: Vec<_> = output.results.into_iter().map(|run| run.stdout).collect();
    assert_eq!(stdouts, vec![bmf(1.0).stdout, bmf(2.0).stdout]);
    assert_eq!(output.error.as_deref(), Some("the runner stopped"));
}

/// A failed execution fails its job under the same database write lock, so no claim finds the
/// job between the 2 with an iteration to send.
///
/// The test holds the database's write lock while the failed result and then a second runner's
/// claim wait for it. The lock goes to them in the order they asked, so the claim runs right
/// after the result.
///
/// Kills: a server that records the failed execution, releases the lock, and fails the job later.
#[tokio::test]
async fn no_claim_runs_between_a_failed_execution_and_its_job() {
    let lab = Lab::new("fail-gap").await;
    let job = lab.job(Job::plus().iterations(3));
    let mut first = lab.runner("First").await;

    let execution = first.claim().await;
    first.running().await;
    go_silent().await;
    let mut first = first.reconnect(&lab).await;
    let mut second = lab.runner("Second").await;

    // The second runner claims at once and then once a second, until its poll ends.
    second.ready(5).await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let lock = lab.server.context().database.connection.lock().await;
    first
        .send_only(RunnerMessage::Failed {
            job: execution.uuid,
            results: vec![exit(1)],
            error: "Benchmark exited with non-zero exit code: 1".to_owned(),
        })
        .await;
    // The result is stored before it waits for the lock.
    lab.wait_for_execution_output(execution.uuid).await;
    // The second runner's next claim waits behind it.
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    drop(lock);

    let ack = first.recv().await;
    assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");
    let reply = second.recv().await;
    assert!(
        matches!(reply, ServerMessage::NoJob),
        "no claim sees the job between its failed execution and its outcome: {reply:?}"
    );
    assert_eq!(get_status(&lab.server, job), JobStatus::Failed);
}

/// A failed execution and the end of its job commit together. When the job cannot end, the
/// execution stays in flight, and the runner's resend fails the job.
///
/// Kills: a server that commits the failed execution before its job ends, so a crash between the
/// 2 leaves the next iteration to a claim that comes before startup recovery.
#[tokio::test]
async fn a_failed_execution_commits_with_the_end_of_its_job() {
    let lab = Lab::new("fail-atomic").await;
    let job = lab.job(Job::plus().iterations(3));
    let mut runner = lab.runner("Runner").await;
    let execution = runner.claim().await;
    runner.running().await;
    let failed = RunnerMessage::Failed {
        job: execution.uuid,
        results: vec![exit(1)],
        error: "Benchmark exited with non-zero exit code: 1".to_owned(),
    };

    lab.fail_job_status_changes(true);
    runner.send_only(failed.clone()).await;
    runner.closed().await;
    lab.fail_job_status_changes(false);

    assert_eq!(
        lab.execution_status(execution.uuid),
        Some(ExecutionState::RUNNING)
    );
    assert_eq!(get_status(&lab.server, job), JobStatus::Running);
    let mut runner = runner.reconnect(&lab).await;
    runner.resend(failed).await;
    assert_eq!(get_status(&lab.server, job), JobStatus::Failed);
    assert!(runner.poll().await.is_none(), "no second execution");
}

/// A canceled execution and the end of its job commit together, as a failed one does.
///
/// Kills: a server that commits the canceled execution before its job ends.
#[tokio::test]
async fn a_canceled_execution_commits_with_the_end_of_its_job() {
    let lab = Lab::new("cancel-atomic").await;
    let job = lab.job(Job::plus().iterations(3));
    let mut runner = lab.runner("Runner").await;
    let execution = runner.claim().await;
    runner.running().await;
    let canceled = RunnerMessage::Canceled {
        job: execution.uuid,
    };

    lab.fail_job_status_changes(true);
    runner.send_only(canceled.clone()).await;
    runner.closed().await;
    lab.fail_job_status_changes(false);

    assert_eq!(
        lab.execution_status(execution.uuid),
        Some(ExecutionState::RUNNING)
    );
    assert_eq!(get_status(&lab.server, job), JobStatus::Running);
    let mut runner = runner.reconnect(&lab).await;
    runner.resend(canceled).await;
    assert_eq!(get_status(&lab.server, job), JobStatus::Canceled);
    assert!(runner.poll().await.is_none(), "no second execution");
}

/// A job's outcome and the cancel of its executions in flight commit together, also for a caller
/// outside any transaction: a heartbeat that finds the job out of time.
///
/// Kills: an end of a job that commits its outcome before it cancels its executions in flight, so
/// a failure between the 2 leaves executions in flight under a job that has its outcome.
#[tokio::test]
async fn a_job_ends_with_its_executions_in_flight() {
    let lab = Lab::without_heartbeat_timeout("end-atomic").await;
    let job = lab.job(Job::plus().iterations(2).timeout(100));
    let mut first = lab.runner("First").await;
    let mut second = lab.runner("Second").await;
    first.claim().await;
    first.running().await;
    second.claim().await;
    second.running().await;

    lab.fail_execution_cancels(true);
    // 180 seconds in all, past 100 plus the 60 of grace.
    lab.advance(90);
    first.send_only(RunnerMessage::Heartbeat).await;
    first.closed().await;
    lab.fail_execution_cancels(false);

    assert_eq!(get_status(&lab.server, job), JobStatus::Running);
    let statuses: Vec<i32> = lab
        .executions(job)
        .into_iter()
        .map(|(_, _, status)| status)
        .collect();
    assert_eq!(
        statuses,
        vec![ExecutionState::RUNNING, ExecutionState::RUNNING]
    );
}

/// Each execution runs with what is left of the job's timeout, and a job whose time runs out
/// before its last iteration fails, naming how many iterations ran, and gets no next execution.
///
/// Kills: a budget of the whole timeout for every execution, and a job that keeps going past it.
#[tokio::test]
async fn the_budget_shrinks_and_ends_the_job() {
    let lab = Lab::new("budget").await;
    let job = lab.job(Job::plus().iterations(3).timeout(100));
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    assert_eq!(u32::from(execution.config.timeout), 100);
    assert_eq!(u32::from(execution.timeout), 100);
    runner.running().await;
    lab.advance(30);
    runner.complete(execution.uuid, 1.0).await;

    let execution = runner.claim().await;
    assert_eq!(
        u32::from(execution.config.timeout),
        70,
        "100 less the 30 used"
    );
    assert_eq!(u32::from(execution.timeout), 70);
    runner.running().await;
    // Over its budget, but within the grace period.
    lab.advance(75);
    runner.complete(execution.uuid, 2.0).await;

    assert_eq!(get_status(&lab.server, job), JobStatus::Failed);
    assert!(runner.poll().await.is_none(), "no third execution");
    let error = lab
        .output(job)
        .await
        .and_then(|output| output.error)
        .expect("the job's error is stored");
    assert!(error.contains("2 of 3"), "{error}");
}

/// A silent execution goes `Unknown` and nothing more is sent for its job while it is; its result
/// resent after a reconnect lands, and the job goes on with what is left of its time.
///
/// Kills: a server that sends the next iteration while one is lost, and one that drops the resend.
#[tokio::test]
async fn a_silent_execution_holds_its_job_until_its_result_lands() {
    let lab = Lab::new("silent-holds").await;
    let job = lab.job(Job::plus().iterations(2).timeout(100));
    let mut first = lab.runner("First").await;

    let execution = first.claim().await;
    first.running().await;
    go_silent().await;
    assert_eq!(
        lab.execution_status(execution.uuid),
        Some(ExecutionState::UNKNOWN)
    );
    // A channel left idle past the heartbeat timeout is closed, so the second runner connects now.
    let mut second = lab.runner("Second").await;
    assert!(second.poll().await.is_none(), "nothing more is sent");

    lab.advance(50);
    let mut first = first.reconnect(&lab).await;
    first
        .resend(RunnerMessage::Completed {
            job: execution.uuid,
            results: vec![bmf(1.0)],
        })
        .await;
    let execution = second.claim().await;
    assert_eq!(u32::from(execution.timeout), 50, "the silent time counts");
    second.run(execution.uuid, 2.0).await;

    assert_eq!(get_status(&lab.server, job), JobStatus::Processed);
    assert_eq!(lab.stored(job), vec![(0, 1.0), (1, 2.0)]);
}

/// While 1 execution of a job is silent and another still runs, the job stays `Running`, and
/// nothing more is sent for it.
///
/// Kills: a claim that checks only the job's status, which the live execution keeps `Running`.
#[tokio::test]
async fn a_silent_execution_holds_its_job_while_another_runs() {
    let lab = Lab::new("silent-beside").await;
    let job = lab.job(Job::plus().iterations(3));
    let mut first = lab.runner("First").await;
    let mut second = lab.runner("Second").await;

    let execution_0 = first.claim().await;
    first.running().await;
    let execution_1 = second.claim().await;
    second.running().await;
    first.disconnect().await;
    // Past the heartbeat timeout by both clocks, while the second runner keeps its heartbeat.
    lab.advance(i64::try_from(HEARTBEAT_TIMEOUT.as_secs()).unwrap_or(i64::MAX));
    for _ in 0..4 {
        tokio::time::sleep(HEARTBEAT_TIMEOUT / 4).await;
        let ack = second.send(RunnerMessage::Heartbeat).await;
        assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");
    }
    assert_eq!(
        lab.execution_status(execution_0.uuid),
        Some(ExecutionState::UNKNOWN)
    );
    assert_eq!(
        lab.execution_status(execution_1.uuid),
        Some(ExecutionState::RUNNING)
    );
    assert_eq!(get_status(&lab.server, job), JobStatus::Running);

    let mut third = lab.runner("Third").await;
    assert!(
        third.poll().await.is_none(),
        "nothing more is sent while 1 is silent"
    );
}

/// A silent execution's result resent within its budget plus the grace period completes the job.
#[tokio::test]
async fn a_silent_execution_resent_in_the_grace_period_completes_the_job() {
    let lab = Lab::new("silent-grace").await;
    let job = lab.job(Job::plus().timeout(100));
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    runner.running().await;
    go_silent().await;

    lab.advance(100 + 59);
    let mut runner = runner.reconnect(&lab).await;
    runner
        .resend(RunnerMessage::Completed {
            job: execution.uuid,
            results: vec![bmf(1.0)],
        })
        .await;

    assert_eq!(get_status(&lab.server, job), JobStatus::Processed);
    assert_eq!(lab.stored(job), vec![(0, 1.0)]);
}

/// A resend after the execution's budget plus the grace period does not land, and the job ends.
#[tokio::test]
async fn a_silent_execution_resent_too_late_ends_the_job() {
    let lab = Lab::new("silent-late").await;
    let job = lab.job(Job::plus().timeout(100));
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    runner.running().await;
    go_silent().await;

    lab.advance(100 + 61);
    let mut runner = runner.reconnect(&lab).await;
    runner
        .resend(RunnerMessage::Completed {
            job: execution.uuid,
            results: vec![bmf(1.0)],
        })
        .await;

    assert!(
        get_status(&lab.server, job).has_run(),
        "the job has its outcome"
    );
    assert_ne!(get_status(&lab.server, job), JobStatus::Processed);
    assert_eq!(lab.stored(job), Vec::new());
}

/// When a job fails, its execution in flight on another runner gets `Cancel` on its next
/// heartbeat, and its result is discarded.
#[tokio::test]
async fn a_failed_job_cancels_its_other_execution() {
    let lab = Lab::without_heartbeat_timeout("cancel-other").await;
    let job = lab.job(Job::plus().iterations(2));
    let mut first = lab.runner("First").await;
    let mut second = lab.runner("Second").await;

    let execution_0 = first.claim().await;
    first.running().await;
    let execution_1 = second.claim().await;
    second.running().await;
    second
        .send(RunnerMessage::Failed {
            job: execution_1.uuid,
            results: Vec::new(),
            error: "boom".to_owned(),
        })
        .await;
    assert_eq!(get_status(&lab.server, job), JobStatus::Failed);

    let reply = first.send(RunnerMessage::Heartbeat).await;
    assert!(matches!(reply, ServerMessage::Cancel), "{reply:?}");
    let ack = first
        .send(RunnerMessage::Completed {
            job: execution_0.uuid,
            results: vec![bmf(1.0)],
        })
        .await;
    assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");
    assert_eq!(get_status(&lab.server, job), JobStatus::Failed);
    assert_eq!(lab.stored(job), Vec::new());
}

/// The job's timeout bounds the sum of its executions: 2 executions in flight that together pass
/// the timeout plus the grace period end the job at the next heartbeat, though neither alone does.
///
/// Kills: a heartbeat that checks only the execution's own budget.
#[tokio::test]
async fn the_sum_of_executions_in_flight_ends_the_job_at_a_heartbeat() {
    let lab = Lab::without_heartbeat_timeout("sum-heartbeat").await;
    let job = lab.job(Job::plus().iterations(2).timeout(100));
    let mut first = lab.runner("First").await;
    let mut second = lab.runner("Second").await;

    let execution = first.claim().await;
    first.running().await;
    let other = second.claim().await;
    second.running().await;
    assert_eq!(u32::from(execution.timeout), 100);
    assert_eq!(u32::from(other.timeout), 100, "no time used yet");

    // 90 seconds each, within 100 plus the 60 of grace, and 180 in all, past it.
    lab.advance(90);
    let reply = second.send(RunnerMessage::Heartbeat).await;
    assert!(matches!(reply, ServerMessage::Cancel), "{reply:?}");
    assert_eq!(get_status(&lab.server, job), JobStatus::Canceled);
    let reply = first.send(RunnerMessage::Heartbeat).await;
    assert!(matches!(reply, ServerMessage::Cancel), "{reply:?}");
}

/// 3 executions of 20 seconds each bill 1 minute, not 3, and usage counts 60 seconds.
///
/// Kills: a bill per execution, and a clock that runs between executions.
#[tokio::test]
async fn three_short_executions_bill_one_minute() {
    let lab = Lab::new("billing").await;
    let job = lab.job(Job::plus().iterations(3));
    let mut runner = lab.runner("Runner").await;

    for value in [1.0, 2.0, 3.0] {
        let execution = runner.claim().await;
        runner.running().await;
        lab.advance(10);
        runner.send(RunnerMessage::Heartbeat).await;
        lab.advance(10);
        runner.complete(execution.uuid, value).await;
        // Time between executions is not the job's.
        lab.advance(100);
    }

    assert_eq!(get_status(&lab.server, job), JobStatus::Processed);
    assert_eq!(lab.billed_minutes(job), Some(1));
    assert_eq!(lab.job_duration(job), Some(60));
}

/// 2 executions of 1 job in flight on 2 runners each keep their own heartbeat timeout: when both
/// runners drop, both executions go `Unknown`.
///
/// Kills: timeout tasks keyed by job, where the second replaces the first.
#[tokio::test]
async fn each_execution_keeps_its_own_timeout() {
    let lab = Lab::new("own-timeout").await;
    let job = lab.job(Job::plus().iterations(2));
    let mut first = lab.runner("First").await;
    let mut second = lab.runner("Second").await;

    let execution_0 = first.claim().await;
    first.running().await;
    let execution_1 = second.claim().await;
    second.running().await;
    first.disconnect().await;
    second.disconnect().await;

    // Past the heartbeat timeout by both clocks: the server's, and the timers'.
    lab.advance(i64::try_from(HEARTBEAT_TIMEOUT.as_secs()).unwrap_or(i64::MAX));
    tokio::time::sleep(HEARTBEAT_TIMEOUT).await;
    assert_eq!(
        lab.execution_status(execution_0.uuid),
        Some(ExecutionState::UNKNOWN)
    );
    assert_eq!(
        lab.execution_status(execution_1.uuid),
        Some(ExecutionState::UNKNOWN)
    );
    assert!(!get_status(&lab.server, job).has_run());
}

/// A restart while a job is between executions leaves it `Running`, and its next execution goes
/// out, with the finished one kept.
///
/// Kills: a startup that marks a job with nothing in flight `Unknown`, or sends iteration 0 again.
#[tokio::test]
async fn a_restart_between_executions_keeps_the_job_running() {
    let lab = Lab::new("restart-between").await;
    let job = lab.job(Job::plus().iterations(2));
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    runner.run(execution.uuid, 1.0).await;
    lab.restart().await;
    assert_eq!(get_status(&lab.server, job), JobStatus::Running);

    let execution = runner.claim().await;
    runner.run(execution.uuid, 2.0).await;
    assert_eq!(lab.stored(job), vec![(0, 1.0), (1, 2.0)]);
    assert_eq!(lab.executions(job).len(), 2);
}

/// A restart in the middle of a job keeps its finished executions, takes the result of the one
/// in flight, and sends only the iteration still to run.
#[tokio::test]
async fn a_restart_mid_job_sends_only_what_is_left() {
    let lab = Lab::new("restart-mid").await;
    let job = lab.job(Job::plus().iterations(3));
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    runner.run(execution.uuid, 1.0).await;
    let execution = runner.claim().await;
    runner.running().await;
    lab.restart().await;
    runner.complete(execution.uuid, 2.0).await;
    let execution = runner.claim().await;
    runner.run(execution.uuid, 3.0).await;

    assert_eq!(get_status(&lab.server, job), JobStatus::Processed);
    assert_eq!(lab.stored(job), vec![(0, 1.0), (1, 2.0), (2, 3.0)]);
    assert_eq!(
        lab.executions(job)
            .into_iter()
            .map(|(iteration, _, _)| iteration)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

/// The callback fires once for a job of 3 iterations, when the job has its outcome.
#[tokio::test]
async fn the_callback_fires_once_per_job() {
    let lab = Lab::new("callback").await;
    let job = lab.job(Job::plus().iterations(3));
    insert_pending_callback(&lab.server, job);
    let mut runner = lab.runner("Runner").await;

    for value in [1.0, 2.0, 3.0] {
        let execution = runner.claim().await;
        runner.run(execution.uuid, value).await;
    }

    assert_one_callback(&lab.server, job, "processed").await;
}

// =============================================================================
// Parse on arrival
// =============================================================================

/// A job whose first execution's output Bencher cannot read fails after that execution: no
/// second execution goes out, and the job's error names iteration 0 and the parse error.
///
/// Kills: a server that parses a job's output only once every iteration has run.
#[tokio::test]
async fn an_unreadable_first_execution_fails_the_job() {
    unreadable_first_execution_fails_the_job(Job::plus().iterations(3), "unreadable").await;
}

/// `allow_failure` covers a run that fails, not output Bencher cannot read, so the job still fails
/// after its first execution.
///
/// Kills: a server that makes unreadable output a gap under `allow_failure`.
#[tokio::test]
async fn an_unreadable_first_execution_fails_the_job_under_allow_failure() {
    unreadable_first_execution_fails_the_job(
        Job::plus().iterations(3).allow_failure(),
        "unreadable-allow",
    )
    .await;
}

#[expect(clippy::expect_used, reason = "test helper")]
async fn unreadable_first_execution_fails_the_job(job: Job, label: &str) {
    let lab = Lab::new(label).await;
    let job = lab.job(job);
    let mut runner = lab.runner("Runner").await;
    insert_pending_callback(&lab.server, job);

    let execution = runner.claim().await;
    runner.running().await;
    let ack = runner
        .send(RunnerMessage::Completed {
            job: execution.uuid,
            results: vec![unreadable()],
        })
        .await;
    assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");

    assert_eq!(
        get_status(&lab.server, job),
        JobStatus::Failed,
        "the job fails after its first execution"
    );
    assert!(runner.poll().await.is_none(), "no second execution");
    assert_eq!(lab.stored(job), Vec::new(), "a failed job is not processed");
    assert_eq!(
        lab.parse_outcome(job),
        Some(ParseOutcome::Failed),
        "the report says its results did not parse"
    );
    assert_eq!(
        lab.execution_status(execution.uuid),
        Some(ExecutionState::COMPLETED),
        "the run was clean, so its execution stays completed"
    );
    let output = lab.output(job).await.expect("the job's output is stored");
    let error = output.error.expect("the job's error");
    assert!(
        error.starts_with(
            "Failed to parse the output of iteration 0: Failed to convert results with adapter"
        ),
        "{error}"
    );
    let stdout: Vec<Option<String>> = output.results.into_iter().map(|run| run.stdout).collect();
    assert_eq!(
        stdout,
        vec![unreadable().stdout],
        "the run's output is kept"
    );
    assert_one_callback(&lab.server, job, "failed").await;
}

/// A job whose output parses in every execution stores the report it stored before the parse on
/// arrival: each run once, as its iteration, a parse of `Parsed`, and no warning.
///
/// Kills: a parse on arrival that writes to the report.
#[tokio::test]
async fn a_readable_job_stores_its_report_once() {
    let lab = Lab::new("readable").await;
    let job = lab.job(Job::plus().iterations(3));
    let mut runner = lab.runner("Runner").await;

    for value in [1.0, 2.0, 3.0] {
        let execution = runner.claim().await;
        runner.run(execution.uuid, value).await;
    }

    assert_eq!(get_status(&lab.server, job), JobStatus::Processed);
    assert_eq!(lab.stored(job), vec![(0, 1.0), (1, 2.0), (2, 3.0)]);
    assert_eq!(lab.parse_outcome(job), Some(ParseOutcome::Parsed));
    assert_eq!(lab.warnings(job), 0);
}

/// Under `allow_failure`, a run that fails is a gap, and a gap is not parsed: a run that exited
/// non-zero with output Bencher cannot read, and a run with no result, leave the job to run on.
///
/// Kills: a server that parses the output of a run its job does not keep.
#[tokio::test]
async fn a_gap_is_not_parsed() {
    let lab = Lab::new("gap-unread").await;
    let job = lab.job(Job::plus().iterations(4).allow_failure());
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    runner.run(execution.uuid, 1.0).await;

    let execution = runner.claim().await;
    runner.running().await;
    let mut failed = unreadable();
    failed.exit_code = 1;
    let ack = runner
        .send(RunnerMessage::Completed {
            job: execution.uuid,
            results: vec![failed],
        })
        .await;
    assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");

    let execution = runner.claim().await;
    runner.running().await;
    let ack = runner
        .send(RunnerMessage::Completed {
            job: execution.uuid,
            results: Vec::new(),
        })
        .await;
    assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");

    let execution = runner.claim().await;
    runner.run(execution.uuid, 4.0).await;

    assert_eq!(get_status(&lab.server, job), JobStatus::Processed);
    assert_eq!(lab.stored(job), vec![(0, 1.0), (3, 4.0)]);
}

/// A `Failed` whose output Bencher cannot read fails the job with the runner's error, and its
/// output is not parsed.
///
/// Kills: a parse on arrival that reads a `Failed` execution's output before it fails the job.
#[tokio::test]
async fn a_failed_execution_is_not_parsed() {
    let lab = Lab::new("failed-unread").await;
    let job = lab.job(Job::plus().iterations(3));
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    runner.running().await;
    let ack = runner
        .send(RunnerMessage::Failed {
            job: execution.uuid,
            results: vec![unreadable()],
            error: "the runner stopped".to_owned(),
        })
        .await;
    assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");

    assert_eq!(get_status(&lab.server, job), JobStatus::Failed);
    assert!(runner.poll().await.is_none(), "no second execution");
    let output = lab.output(job).await.expect("the job's output is stored");
    assert_eq!(output.error.as_deref(), Some("the runner stopped"));
    assert_eq!(lab.parse_outcome(job), None);
}

/// A job of 2 files per iteration parses every file of an execution as it arrives: output Bencher
/// cannot read in the second file of iteration 1 fails the job after that execution.
///
/// Kills: a parse on arrival that reads only an execution's first file or its stdout.
#[tokio::test]
async fn an_unreadable_second_file_fails_the_job() {
    let lab = Lab::new("unreadable-file").await;
    let job = lab.job(Job::plus().iterations(3).file_paths(&["a.json", "b.json"]));
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    runner.running().await;
    let ack = runner
        .send(RunnerMessage::Completed {
            job: execution.uuid,
            results: vec![files(&[
                ("a.json", bmf_text(1.0)),
                ("b.json", bmf_text(2.0)),
            ])],
        })
        .await;
    assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");
    assert_eq!(get_status(&lab.server, job), JobStatus::Running);

    let execution = runner.claim().await;
    runner.running().await;
    let ack = runner
        .send(RunnerMessage::Completed {
            job: execution.uuid,
            results: vec![files(&[
                ("a.json", bmf_text(3.0)),
                ("b.json", UNREADABLE.to_owned()),
            ])],
        })
        .await;
    assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");

    assert_eq!(get_status(&lab.server, job), JobStatus::Failed);
    assert!(runner.poll().await.is_none(), "no third execution");
    let output = lab.output(job).await.expect("the job's output is stored");
    let error = output.error.expect("the job's error");
    assert!(
        error.starts_with(
            "Failed to parse the output of iteration 1: Failed to convert results with adapter"
        ),
        "{error}"
    );
}

/// A job at BMF version 1 parses each execution with its report's adapter at version 1: libtest's
/// JSON bench events, which `rust_bench` reads at version 1 only and `json` never reads, complete
/// the job, and each run stores its median and range as its iteration.
///
/// Kills: a parse on arrival at version 0, or with the `json` adapter, in place of the job's.
#[tokio::test]
async fn a_version_1_job_parses_with_its_adapter() {
    let lab = Lab::new("v1-adapter").await;
    let job = lab.job(
        Job::plus()
            .iterations(2)
            .bmf_version(1)
            .adapter(Adapter::RustBench),
    );
    let mut runner = lab.runner("Runner").await;

    for median in [10.0, 20.0] {
        let execution = runner.claim().await;
        runner.running().await;
        let ack = runner
            .send(RunnerMessage::Completed {
                job: execution.uuid,
                results: vec![libtest_bench(median)],
            })
            .await;
        assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");
    }

    assert_eq!(get_status(&lab.server, job), JobStatus::Processed);
    assert_eq!(lab.parse_outcome(job), Some(ParseOutcome::Parsed));
    assert_eq!(
        lab.stored_metrics(job),
        vec![
            (0, "median".to_owned(), 10.0),
            (0, "range".to_owned(), LIBTEST_DEVIATION),
            (1, "median".to_owned(), 20.0),
            (1, "range".to_owned(), LIBTEST_DEVIATION),
        ]
    );
}

/// A job at BMF version 1 whose first execution's output is BMF version 0 fails after that
/// execution, with the refusal the final processing gives for the job's version.
///
/// Kills: a parse on arrival at version 0 in place of the job's.
#[tokio::test]
async fn a_version_1_job_refuses_version_0_output_on_arrival() {
    let lab = Lab::new("v1-refuses-v0").await;
    let job = lab.job(Job::plus().iterations(3).bmf_version(1));
    let mut runner = lab.runner("Runner").await;

    let execution = runner.claim().await;
    runner.running().await;
    let ack = runner
        .send(RunnerMessage::Completed {
            job: execution.uuid,
            results: vec![bmf(1.0)],
        })
        .await;
    assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");

    assert_eq!(
        get_status(&lab.server, job),
        JobStatus::Failed,
        "the job fails after its first execution"
    );
    assert!(runner.poll().await.is_none(), "no second execution");
    let output = lab.output(job).await.expect("the job's output is stored");
    let error = output.error.expect("the job's error");
    assert!(
        error.starts_with(
            "Failed to parse the output of iteration 0: Results parsed as BMF version 0, but the remote job was accepted at BMF version 1. Send BMF version 1 results"
        ),
        "{error}"
    );
}

/// A parse failure on an execution that ends while another of its job's executions is in flight
/// cancels that one, and its later result is discarded.
///
/// Kills: a parse failure that leaves the job's other execution in flight, or takes its result.
#[tokio::test]
async fn an_unreadable_execution_cancels_its_other_execution() {
    let lab = Lab::without_heartbeat_timeout("unreadable-other").await;
    let job = lab.job(Job::plus().iterations(2));
    let mut first = lab.runner("First").await;
    let mut second = lab.runner("Second").await;

    let execution_0 = first.claim().await;
    first.running().await;
    let execution_1 = second.claim().await;
    second.running().await;
    let ack = second
        .send(RunnerMessage::Completed {
            job: execution_1.uuid,
            results: vec![unreadable()],
        })
        .await;
    assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");
    assert_eq!(get_status(&lab.server, job), JobStatus::Failed);
    assert_eq!(
        lab.execution_status(execution_0.uuid),
        Some(ExecutionState::CANCELED)
    );

    let reply = first.send(RunnerMessage::Heartbeat).await;
    assert!(matches!(reply, ServerMessage::Cancel), "{reply:?}");
    let ack = first
        .send(RunnerMessage::Completed {
            job: execution_0.uuid,
            results: vec![bmf(1.0)],
        })
        .await;
    assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");
    assert_eq!(get_status(&lab.server, job), JobStatus::Failed);
    assert_eq!(
        lab.execution_status(execution_0.uuid),
        Some(ExecutionState::CANCELED)
    );
    assert_eq!(lab.stored(job), Vec::new());
}

// =============================================================================
// Harness
// =============================================================================

/// The server's heartbeat timeout, plus a margin.
const HEARTBEAT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(7);

/// The stored execution statuses, as the integers the table holds.
struct ExecutionState;

impl ExecutionState {
    const RUNNING: i32 = 1;
    const COMPLETED: i32 = 2;
    const CANCELED: i32 = 4;
    const UNKNOWN: i32 = 5;
}

/// A server with a mock clock, an organization with one project, and one spec.
struct Lab {
    server: TestServer,
    time: Arc<AtomicI64>,
    admin: TestUser,
    label: String,
    project_uuid: bencher_json::ProjectUuid,
    project_id: i32,
    spec_id: i32,
    /// The project's first report, whose head, version, and testbed every job's report shares.
    report_id: i32,
}

/// A job to insert.
#[derive(Clone, Copy)]
struct Job {
    priority: Priority,
    iter: u32,
    timeout: i32,
    allow_failure: bool,
    file_paths: &'static [&'static str],
    /// The job config's BMF version, if it declares one.
    bmf_version: Option<u8>,
    /// The report's adapter, if not the project's first report's.
    adapter: Option<Adapter>,
}

impl Job {
    fn plus() -> Self {
        Self {
            priority: Priority::Plus,
            iter: 1,
            timeout: 3600,
            allow_failure: false,
            file_paths: &[],
            bmf_version: None,
            adapter: None,
        }
    }

    fn free() -> Self {
        Self {
            priority: Priority::Free,
            ..Self::plus()
        }
    }

    fn iterations(self, iter: u32) -> Self {
        Self { iter, ..self }
    }

    fn timeout(self, timeout: i32) -> Self {
        Self { timeout, ..self }
    }

    fn allow_failure(self) -> Self {
        Self {
            allow_failure: true,
            ..self
        }
    }

    fn file_paths(self, file_paths: &'static [&'static str]) -> Self {
        Self { file_paths, ..self }
    }

    fn bmf_version(self, bmf_version: u8) -> Self {
        Self {
            bmf_version: Some(bmf_version),
            ..self
        }
    }

    fn adapter(self, adapter: Adapter) -> Self {
        Self {
            adapter: Some(adapter),
            ..self
        }
    }
}

impl Lab {
    async fn new(label: &str) -> Self {
        Self::with_heartbeat_timeout(label, None).await
    }

    /// A lab whose heartbeat timeout outlasts the test, for a test that leaves an execution in
    /// flight without heartbeats while another runner acts.
    async fn without_heartbeat_timeout(label: &str) -> Self {
        Self::with_heartbeat_timeout(label, Some(std::time::Duration::from_hours(1))).await
    }

    async fn with_heartbeat_timeout(
        label: &str,
        heartbeat_timeout: Option<std::time::Duration>,
    ) -> Self {
        let time = Arc::new(AtomicI64::new(DateTime::TEST.timestamp()));
        let time_ref = time.clone();
        let clock = bencher_json::Clock::Custom(Arc::new(move || {
            DateTime::try_from(time_ref.load(Ordering::Relaxed)).unwrap_or(DateTime::TEST)
        }));
        let server = TestServer::new_with_options(TestServerOptions {
            upload_timeout: Some(3600),
            max_body_size: Some(1024 * 1024),
            clock: Some(clock),
            heartbeat_timeout,
            ..TestServerOptions::default()
        })
        .await;
        let admin = server
            .signup("Admin", &format!("dispatch-{label}@example.com"))
            .await;
        let org = server
            .create_org(&admin, &format!("Dispatch {label}"))
            .await;
        let project = server
            .create_project(&admin, &org, &format!("Dispatch {label}"))
            .await;
        let project_id = get_project_id(&server, project.slug.as_ref());
        let (_, spec_id) = insert_test_spec(&server);
        let report_id = create_test_report(&server, project_id);
        Self {
            server,
            time,
            admin,
            label: label.to_owned(),
            project_uuid: project.uuid,
            project_id,
            spec_id,
            report_id,
        }
    }

    fn advance(&self, secs: i64) {
        self.time.fetch_add(secs, Ordering::Relaxed);
    }

    /// Insert a pending job with its own report, created 1 second after the last.
    #[expect(clippy::expect_used, reason = "test helper")]
    fn job(&self, job: Job) -> JobUuid {
        let Job {
            priority,
            iter,
            timeout,
            allow_failure,
            file_paths,
            bmf_version,
            adapter,
        } = job;
        let report_id = create_sibling_report(&self.server, self.report_id);
        let mut conn = self.server.db_conn();
        let organization_id: i32 = schema::project::table
            .filter(schema::project::id.eq(self.project_id))
            .select(schema::project::organization_id)
            .first(&mut conn)
            .expect("Failed to get the organization");
        let created: i64 = schema::job::table
            .count()
            .get_result(&mut conn)
            .expect("Failed to count jobs");
        let created = DateTime::try_from(DateTime::TEST.timestamp() - 1000 + created)
            .expect("Invalid timestamp");
        let uuid = JobUuid::new();
        let mut config = serde_json::json!({
            "registry": "https://registry.bencher.dev",
            "project": self.project_uuid,
            "digest": format!("sha256:{}", "0".repeat(64)),
            "timeout": timeout,
            "iter": iter,
            "allow_failure": allow_failure,
            "file_paths": (!file_paths.is_empty()).then_some(file_paths),
        });
        if let Some(bmf_version) = bmf_version
            && let Some(object) = config.as_object_mut()
        {
            object.insert("bmf_version".to_owned(), serde_json::json!(bmf_version));
        }
        diesel::insert_into(schema::job::table)
            .values((
                schema::job::uuid.eq(uuid),
                schema::job::report_id.eq(report_id),
                schema::job::project_id.eq(self.project_id),
                schema::job::organization_id.eq(organization_id),
                schema::job::source_ip.eq(TEST_SOURCE_IP),
                schema::job::status.eq(JobStatus::Pending),
                schema::job::spec_id.eq(self.spec_id),
                schema::job::config.eq(config.to_string()),
                schema::job::timeout.eq(timeout),
                schema::job::priority.eq(priority),
                schema::job::created.eq(created),
                schema::job::modified.eq(created),
            ))
            .execute(&mut conn)
            .expect("Failed to insert the job");
        diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
            .set(schema::report::spec_id.eq(Some(self.spec_id)))
            .execute(&mut conn)
            .expect("Failed to set the report's spec");
        if let Some(adapter) = adapter {
            diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
                .set(schema::report::adapter.eq(adapter))
                .execute(&mut conn)
                .expect("Failed to set the report's adapter");
        }
        uuid
    }

    /// Make the job older than every other, so it goes out first in its tier.
    #[expect(clippy::expect_used, reason = "test helper")]
    fn make_oldest(&self, job: JobUuid) {
        let created =
            DateTime::try_from(DateTime::TEST.timestamp() - 2000).expect("Invalid timestamp");
        diesel::update(schema::job::table.filter(schema::job::uuid.eq(job)))
            .set(schema::job::created.eq(created))
            .execute(&mut self.server.db_conn())
            .expect("Failed to make the job the oldest");
    }

    async fn runner(&self, name: &str) -> FakeRunner {
        let runner = create_runner(
            &self.server,
            &self.admin.token,
            &format!("{name} {}", self.label),
        )
        .await;
        let id = get_runner_id(&self.server, runner.uuid);
        associate_runner_spec(&self.server, id, self.spec_id);
        let key = runner.key.to_string();
        let ws = connect_channel_ws(&self.server, runner.uuid, &key).await;
        FakeRunner {
            ws,
            uuid: runner.uuid,
            key,
            id,
        }
    }

    /// What a server restart runs: the job recovery at startup.
    async fn restart(&self) {
        let log = slog::Logger::root(slog::Discard, slog::o!());
        spawn_job_recovery(&log, self.server.context()).await;
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn job_report(&self, job: JobUuid) -> i32 {
        schema::job::table
            .filter(schema::job::uuid.eq(job))
            .select(schema::job::report_id)
            .first(&mut self.server.db_conn())
            .expect("Failed to get the job's report")
    }

    /// Every value the job's report stored, as its iteration and value, in order.
    #[expect(clippy::expect_used, reason = "test helper")]
    fn stored(&self, job: JobUuid) -> Vec<(i32, f64)> {
        schema::metric::table
            .inner_join(schema::report_benchmark::table)
            .filter(schema::report_benchmark::report_id.eq(self.job_report(job)))
            .select((schema::report_benchmark::iteration, schema::metric::value))
            .order(schema::report_benchmark::iteration)
            .load(&mut self.server.db_conn())
            .expect("Failed to load the stored values")
    }

    /// Every metric the job's report stored, as its iteration, metric name, and value, in order.
    #[expect(clippy::expect_used, reason = "test helper")]
    fn stored_metrics(&self, job: JobUuid) -> Vec<(i32, String, f64)> {
        schema::metric::table
            .inner_join(schema::report_benchmark::table)
            .filter(schema::report_benchmark::report_id.eq(self.job_report(job)))
            .select((
                schema::report_benchmark::iteration,
                schema::metric::name,
                schema::metric::value,
            ))
            .order((schema::report_benchmark::iteration, schema::metric::name))
            .load(&mut self.server.db_conn())
            .expect("Failed to load the stored metrics")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn parse_outcome(&self, job: JobUuid) -> Option<ParseOutcome> {
        schema::report::table
            .filter(schema::report::id.eq(self.job_report(job)))
            .select(schema::report::parse_outcome)
            .first(&mut self.server.db_conn())
            .expect("Failed to get the report's parse")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn warnings(&self, job: JobUuid) -> i64 {
        schema::report_warning::table
            .filter(schema::report_warning::report_id.eq(self.job_report(job)))
            .count()
            .get_result(&mut self.server.db_conn())
            .expect("Failed to count the report's warnings")
    }

    async fn output(&self, job: JobUuid) -> Option<bencher_json::runner::JsonJobOutput> {
        self.server
            .context()
            .oci_storage()
            .job_output()
            .get(self.project_uuid, job)
            .await
            .ok()
            .flatten()
    }

    /// Wait, up to 10 seconds, until the execution's own output is stored.
    #[expect(clippy::panic, reason = "test helper")]
    async fn wait_for_execution_output(&self, execution: JobUuid) {
        for _ in 0..1000 {
            let stored = self
                .server
                .context()
                .oci_storage()
                .job_output()
                .get_execution(self.project_uuid, execution)
                .await
                .ok()
                .flatten();
            if stored.is_some() {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("The execution's output was never stored");
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn job_runner(&self, job: JobUuid) -> Option<i32> {
        schema::job::table
            .filter(schema::job::uuid.eq(job))
            .select(schema::job::runner_id)
            .first(&mut self.server.db_conn())
            .expect("Failed to get the job's runner")
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn billed_minutes(&self, job: JobUuid) -> Option<i32> {
        schema::job::table
            .filter(schema::job::uuid.eq(job))
            .select(schema::job::last_billed_minute)
            .first(&mut self.server.db_conn())
            .expect("Failed to get the job's bill")
    }

    fn job_duration(&self, job: JobUuid) -> Option<i32> {
        schema::job_duration_by_report::table
            .filter(schema::job_duration_by_report::report_id.eq(self.job_report(job)))
            .select(schema::job_duration_by_report::job_duration)
            .first(&mut self.server.db_conn())
            .ok()
    }

    /// The job an execution belongs to.
    #[expect(clippy::expect_used, reason = "test helper")]
    fn execution_job(&self, execution: JobUuid) -> JobUuid {
        #[derive(diesel::QueryableByName)]
        struct Row {
            #[diesel(sql_type = diesel::sql_types::Text)]
            uuid: String,
        }
        let row = diesel::sql_query(
            "SELECT job.uuid FROM job_execution INNER JOIN job ON job.id = job_execution.job_id
                WHERE job_execution.uuid = ?",
        )
        .bind::<diesel::sql_types::Text, _>(execution.to_string())
        .get_result::<Row>(&mut self.server.db_conn())
        .expect("Failed to load the execution's job");
        row.uuid.parse().expect("Invalid job UUID")
    }

    /// The job's executions, as their iteration, runner, and status, in iteration order.
    #[expect(clippy::expect_used, reason = "test helper")]
    fn executions(&self, job: JobUuid) -> Vec<(i32, i32, i32)> {
        #[derive(diesel::QueryableByName)]
        struct Row {
            #[diesel(sql_type = diesel::sql_types::Integer)]
            iteration: i32,
            #[diesel(sql_type = diesel::sql_types::Integer)]
            runner_id: i32,
            #[diesel(sql_type = diesel::sql_types::Integer)]
            status: i32,
        }
        diesel::sql_query(
            "SELECT job_execution.iteration, job_execution.runner_id, job_execution.status
                FROM job_execution INNER JOIN job ON job.id = job_execution.job_id
                WHERE job.uuid = ? ORDER BY job_execution.iteration",
        )
        .bind::<diesel::sql_types::Text, _>(job.to_string())
        .load::<Row>(&mut self.server.db_conn())
        .expect("Failed to load the executions")
        .into_iter()
        .map(|row| (row.iteration, row.runner_id, row.status))
        .collect()
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    fn execution_status(&self, execution: JobUuid) -> Option<i32> {
        #[derive(diesel::QueryableByName)]
        struct Row {
            #[diesel(sql_type = diesel::sql_types::Integer)]
            status: i32,
        }
        diesel::sql_query("SELECT status FROM job_execution WHERE uuid = ?")
            .bind::<diesel::sql_types::Text, _>(execution.to_string())
            .load::<Row>(&mut self.server.db_conn())
            .expect("Failed to load the execution")
            .first()
            .map(|row| row.status)
    }

    /// Make every change of a job's status fail, or stop that.
    #[expect(clippy::expect_used, reason = "test helper")]
    fn fail_job_status_changes(&self, fail: bool) {
        let sql = if fail {
            "CREATE TRIGGER fail_job_status BEFORE UPDATE OF status ON job
                WHEN NEW.status <> OLD.status
                BEGIN SELECT RAISE(ABORT, 'injected failure'); END"
        } else {
            "DROP TRIGGER fail_job_status"
        };
        diesel::sql_query(sql)
            .execute(&mut self.server.db_conn())
            .expect("Failed to toggle the trigger");
    }

    /// Make every cancel of an execution fail, or stop that.
    #[expect(clippy::expect_used, reason = "test helper")]
    fn fail_execution_cancels(&self, fail: bool) {
        let sql = if fail {
            "CREATE TRIGGER fail_execution_cancel BEFORE UPDATE OF status ON job_execution
                WHEN NEW.status = 4
                BEGIN SELECT RAISE(ABORT, 'injected failure'); END"
        } else {
            "DROP TRIGGER fail_execution_cancel"
        };
        diesel::sql_query(sql)
            .execute(&mut self.server.db_conn())
            .expect("Failed to toggle the trigger");
    }
}

/// A runner that speaks the channel protocol by hand.
struct FakeRunner {
    ws: WsStream,
    uuid: RunnerUuid,
    key: String,
    id: i32,
}

impl FakeRunner {
    /// Send `Ready` and return the job the server sends, if any, within 1 second.
    #[expect(clippy::panic, reason = "test helper")]
    async fn poll(&mut self) -> Option<JsonClaimedJob> {
        self.ready(1).await;
        match self.recv().await {
            ServerMessage::Job(job) => Some(*job),
            ServerMessage::NoJob => None,
            other @ (ServerMessage::Ack { .. }
            | ServerMessage::Cancel
            | ServerMessage::Update { .. }) => panic!("Expected Job or NoJob, got: {other:?}"),
        }
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn claim(&mut self) -> JsonClaimedJob {
        self.poll().await.expect("Expected a job")
    }

    /// Send `Ready` with a poll of `secs` seconds, and do not wait for the reply.
    #[expect(clippy::expect_used, reason = "test helper")]
    async fn ready(&mut self, secs: u32) {
        let ready = RunnerMessage::Ready(JsonReady::new(
            Some(PollTimeout::try_from(secs).expect("Invalid poll timeout")),
            Some(runner_metadata()),
        ));
        self.send_only(ready).await;
    }

    async fn send(&mut self, msg: RunnerMessage) -> ServerMessage {
        self.send_only(msg).await;
        self.recv().await
    }

    async fn send_only(&mut self, msg: RunnerMessage) {
        send_runner_msg(&mut self.ws, &msg).await;
    }

    async fn recv(&mut self) -> ServerMessage {
        recv_server_msg(&mut self.ws).await
    }

    async fn running(&mut self) {
        let ack = self.send(RunnerMessage::Running).await;
        assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");
    }

    async fn complete(&mut self, execution: JobUuid, value: f64) {
        let ack = self
            .send(RunnerMessage::Completed {
                job: execution,
                results: vec![bmf(value)],
            })
            .await;
        assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");
    }

    async fn run(&mut self, execution: JobUuid, value: f64) {
        self.running().await;
        self.complete(execution, value).await;
    }

    /// Send a result outside of an execution, as a runner does after it reconnects.
    async fn resend(&mut self, msg: RunnerMessage) {
        let ack = self.send(msg).await;
        assert!(matches!(ack, ServerMessage::Ack { .. }), "{ack:?}");
    }

    /// Claim and run every job there is, and return the job of each execution in turn.
    async fn run_all(&mut self, lab: &Lab) -> Vec<JobUuid> {
        let mut jobs = Vec::new();
        while let Some(execution) = self.poll().await {
            jobs.push(lab.execution_job(execution.uuid));
            self.run(execution.uuid, 1.0).await;
        }
        jobs
    }

    /// Wait, up to 10 seconds, until the server closes the channel.
    async fn closed(&mut self) {
        let next = tokio::time::timeout(std::time::Duration::from_secs(10), self.ws.next()).await;
        assert!(
            matches!(next, Ok(None | Some(Ok(Message::Close(_)) | Err(_)))),
            "Expected the channel to close, got: {next:?}"
        );
    }

    #[expect(clippy::expect_used, reason = "test helper")]
    async fn disconnect(&mut self) {
        self.ws.close(None).await.expect("Failed to close");
    }

    /// A new channel for the same runner.
    async fn reconnect(&self, lab: &Lab) -> Self {
        Self {
            ws: connect_channel_ws(&lab.server, self.uuid, &self.key).await,
            uuid: self.uuid,
            key: self.key.clone(),
            id: self.id,
        }
    }
}

/// Send nothing, until the server's heartbeat timeout closes a silent runner's channel.
async fn go_silent() {
    tokio::time::sleep(HEARTBEAT_TIMEOUT).await;
}

/// A run's BMF output on stdout: one benchmark with one value.
fn bmf(value: f64) -> JsonIterationOutput {
    JsonIterationOutput {
        exit_code: 0,
        stdout: Some(bmf_text(value)),
        stderr: None,
        output: None,
    }
}

fn bmf_text(value: f64) -> String {
    serde_json::json!({ "bench": { "latency": { "value": value } } }).to_string()
}

/// The deviation of every libtest bench event, which `rust_bench` stores as its range.
const LIBTEST_DEVIATION: f64 = 0.5;

/// A clean run whose stdout is 1 bench event of nightly libtest's `--format json`.
fn libtest_bench(median: f64) -> JsonIterationOutput {
    let event = serde_json::json!({
        "type": "bench",
        "name": "fib_10",
        "median": median,
        "deviation": LIBTEST_DEVIATION,
    });
    JsonIterationOutput {
        exit_code: 0,
        stdout: Some(event.to_string()),
        stderr: None,
        output: None,
    }
}

/// Output that no adapter reads.
const UNREADABLE: &str = "this is not benchmark output";

/// A clean run whose stdout Bencher cannot read.
fn unreadable() -> JsonIterationOutput {
    JsonIterationOutput {
        exit_code: 0,
        stdout: Some(UNREADABLE.to_owned()),
        stderr: None,
        output: None,
    }
}

/// A clean run that wrote these files.
fn files(files: &[(&str, String)]) -> JsonIterationOutput {
    JsonIterationOutput {
        exit_code: 0,
        stdout: None,
        stderr: None,
        output: Some(
            files
                .iter()
                .map(|(path, contents)| ((*path).into(), contents.clone()))
                .collect(),
        ),
    }
}

fn exit(code: i32) -> JsonIterationOutput {
    JsonIterationOutput {
        exit_code: code,
        stdout: None,
        stderr: None,
        output: None,
    }
}
