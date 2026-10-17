//! Runner jobs in every state. A runner job's report is created through the
//! API, but no API takes a job through its states without a runner, so each
//! job row is written into the API's database as the runner protocol leaves it.

use bencher_json::{DateTime, JobUuid, JsonOrganization, JsonProject};
use camino::Utf8Path;
use diesel::{Connection as _, RunQueryDsl as _, SqliteConnection};

use super::seed::{Api, KIB_1, KIB_64, MAIN, Method, NOW, REPORT_DURATION, TESTBED};

const RUNNER: &str = "Curly Fries";
const HELD: &str = "Steak Fries";
const HANDOVER: &str = "Shoestring";
/// Projects the suite changes as it reads them come one for each repeat of a test, so
/// repeats running side by side never meet.
const REPEATS: usize = 4;

/// The spec every job here ran on.
const SPEC: &str = "standard-x86";
const GIB: u64 = 1 << 30;

/// A run that waits on a runner: one report landed with an alert, one runs, one is queued.
pub(super) const JOBS_HASH: &str = "4be71c0";
/// A run whose one report waits for a runner.
const QUEUED_HASH: &str = "9a7e3f2";
/// A run whose one report failed 42 s after it started.
const FAILED_HASH: &str = "e1f0b2d";
/// A run whose one report was canceled before a runner took it.
const CANCELED_HASH: &str = "c4ce1ed";
/// A run its own CI job posted, finished.
const DONE_HASH: &str = "5f3c9a0";
/// The held lines' run: one report raised an alert, one still runs.
pub(super) const HELD_HASH: &str = "b52e7d4";

const MINUTE: i64 = 60;

/// A job's state as the API stores it.
#[derive(Clone, Copy)]
enum Status {
    Pending,
    Running,
    Processed,
    Failed,
    Canceled,
}

impl Status {
    const fn stored(self) -> i32 {
        match self {
            Self::Pending => 0,
            Self::Running => 2,
            Self::Processed => 4,
            Self::Failed => 5,
            Self::Canceled => 6,
        }
    }
}

struct Job {
    status: Status,
    created: i64,
    started: Option<i64>,
    completed: Option<i64>,
}

const fn job(status: Status, created: i64, started: Option<i64>, completed: Option<i64>) -> Job {
    Job {
        status,
        created,
        started,
        completed,
    }
}

/// A latency over the projects' static threshold, which alerts with no history.
const OVER: f64 = 20.0;
const UNDER: f64 = 5.0;

fn threshold() -> serde_json::Value {
    serde_json::json!({
        "models": [{
            "measure": "latency",
            "metric": "value",
            "model": { "test": "static", "upper_boundary": 10.0 },
        }]
    })
}

/// One latency for each of blake3's two sizes.
fn latencies(small: f64, large: f64) -> String {
    serde_json::json!({
        "blake3": [
            { "parameters": { "input_bytes": KIB_1 }, "measures": { "latency": { "value": small } } },
            { "parameters": { "input_bytes": KIB_64 }, "measures": { "latency": { "value": large } } },
        ],
    })
    .to_string()
}

fn full_hash(short: &str) -> String {
    format!("{short}{}", "0".repeat(40 - short.len()))
}

/// A report on `main`; a runner job's report starts with no results.
fn report(start: i64, short: &str, results: Option<String>) -> anyhow::Result<serde_json::Value> {
    let landed = results.is_some();
    let mut report = serde_json::json!({
        "branch": MAIN,
        "hash": full_hash(short),
        "testbed": TESTBED,
        "start_time": DateTime::try_from(start)?,
        "end_time": DateTime::try_from(if landed { start + REPORT_DURATION } else { start })?,
        "results": results.into_iter().collect::<Vec<_>>(),
        "settings": { "adapter": "json" },
    });
    if landed && let Some(object) = report.as_object_mut() {
        object.insert("thresholds".to_owned(), threshold());
    }
    Ok(report)
}

/// The projects whose reports are runner jobs'.
pub(super) struct JobProjects {
    pub runner: JsonProject,
    pub held: Vec<JsonProject>,
    pub handover: Vec<JsonProject>,
}

pub(super) async fn seed(
    api: &Api,
    db: &Utf8Path,
    organization: &JsonOrganization,
    token: &str,
    admin_token: &str,
) -> anyhow::Result<JobProjects> {
    spec(api, admin_token).await?;
    let runner = project(api, organization, RUNNER, token, admin_token).await?;
    runner_runs(api, db, &runner, token).await?;
    let mut held = Vec::with_capacity(REPEATS);
    let mut handover = Vec::with_capacity(REPEATS);
    for repeat in 0..REPEATS {
        let name = format!("{HELD} {repeat}");
        let seeded = project(api, organization, &name, token, admin_token).await?;
        held_run(api, db, &seeded, token).await?;
        held.push(seeded);
        let name = format!("{HANDOVER} {repeat}");
        let seeded = project(api, organization, &name, token, admin_token).await?;
        handover_run(api, db, &seeded, token).await?;
        handover.push(seeded);
    }
    Ok(JobProjects {
        runner,
        held,
        handover,
    })
}

/// A public version 1 project.
async fn project(
    api: &Api,
    organization: &JsonOrganization,
    name: &str,
    token: &str,
    admin_token: &str,
) -> anyhow::Result<JsonProject> {
    let project = api.project(organization, name, "public", token).await?;
    let path = format!("/v0/projects/{}", project.slug);
    api.send(
        Method::Patch,
        &path,
        admin_token,
        &serde_json::json!({ "bmf_version": 1 }),
    )
    .await
}

/// The spec the jobs ran on, made through the API by the server's admin.
async fn spec(api: &Api, admin_token: &str) -> anyhow::Result<()> {
    let _spec: serde_json::Value = api
        .send(
            Method::Post,
            "/v0/specs",
            admin_token,
            &serde_json::json!({
                "name": "Standard x86",
                "slug": SPEC,
                "os": "linux",
                "architecture": "x86_64",
                "cpu": 8,
                "memory": 16 * GIB,
                "disk": 100 * GIB,
            }),
        )
        .await?;
    Ok(())
}

/// Runs whose jobs wait, run, failed, and were canceled, and one finished run.
async fn runner_runs(
    api: &Api,
    db: &Utf8Path,
    project: &JsonProject,
    token: &str,
) -> anyhow::Result<()> {
    let poster = Poster {
        api,
        project,
        token,
    };
    // A threshold checks against history, so the finished run comes first.
    let _done = poster
        .post(NOW - 180 * MINUTE, DONE_HASH, Some(latencies(UNDER, UNDER)))
        .await?;
    let landed = poster
        .post(NOW - 10 * MINUTE, JOBS_HASH, Some(latencies(UNDER, OVER)))
        .await?;
    let running = poster.post(NOW - 9 * MINUTE, JOBS_HASH, None).await?;
    let queued = poster.post(NOW - 30, JOBS_HASH, None).await?;
    let waiting = poster.post(NOW - 20 * MINUTE, QUEUED_HASH, None).await?;
    let failed = poster.post(NOW - 60 * MINUTE, FAILED_HASH, None).await?;
    let canceled = poster.post(NOW - 120 * MINUTE, CANCELED_HASH, None).await?;
    give_jobs(
        db,
        &[
            (
                &landed,
                job(
                    Status::Processed,
                    NOW - 10 * MINUTE,
                    Some(NOW - 10 * MINUTE),
                    Some(NOW - 8 * MINUTE),
                ),
            ),
            (
                &running,
                job(Status::Running, NOW - 9 * MINUTE, Some(NOW - 48), None),
            ),
            (&queued, job(Status::Pending, NOW - 30, None, None)),
            (
                &waiting,
                job(Status::Pending, NOW - 20 * MINUTE, None, None),
            ),
            (
                &failed,
                job(
                    Status::Failed,
                    NOW - 60 * MINUTE,
                    Some(NOW - 60 * MINUTE + 5),
                    Some(NOW - 60 * MINUTE + 42),
                ),
            ),
            (
                &canceled,
                job(
                    Status::Canceled,
                    NOW - 120 * MINUTE,
                    None,
                    Some(NOW - 110 * MINUTE),
                ),
            ),
        ],
    )
}

/// A run that raised one alert and still runs, which the suite adds alerting reports to.
async fn held_run(
    api: &Api,
    db: &Utf8Path,
    project: &JsonProject,
    token: &str,
) -> anyhow::Result<()> {
    let poster = Poster {
        api,
        project,
        token,
    };
    let _history = poster
        .post(NOW - 180 * MINUTE, DONE_HASH, Some(latencies(UNDER, UNDER)))
        .await?;
    let _alerting = poster
        .post(NOW - 5 * MINUTE, HELD_HASH, Some(latencies(UNDER, OVER)))
        .await?;
    let running = poster.post(NOW - 4 * MINUTE, HELD_HASH, None).await?;
    give_jobs(
        db,
        &[(
            &running,
            job(
                Status::Running,
                NOW - 4 * MINUTE,
                Some(NOW - 3 * MINUTE),
                None,
            ),
        )],
    )
}

/// A queued job's report on a branch the suite unarchives to hand the setup card over to it.
async fn handover_run(
    api: &Api,
    db: &Utf8Path,
    project: &JsonProject,
    token: &str,
) -> anyhow::Result<()> {
    let poster = Poster {
        api,
        project,
        token,
    };
    let queued = poster.post(NOW - MINUTE, QUEUED_HASH, None).await?;
    give_jobs(
        db,
        &[(&queued, job(Status::Pending, NOW - MINUTE, None, None))],
    )?;
    let path = format!("/v0/projects/{}/branches/{MAIN}", project.slug);
    let _branch: serde_json::Value = api
        .send(
            Method::Patch,
            &path,
            token,
            &serde_json::json!({ "archived": true }),
        )
        .await?;
    Ok(())
}

/// Posts a project's reports.
struct Poster<'a> {
    api: &'a Api,
    project: &'a JsonProject,
    token: &'a str,
}

impl Poster<'_> {
    /// A report on `main` at `start`, returning its UUID.
    async fn post(
        &self,
        start: i64,
        short: &str,
        results: Option<String>,
    ) -> anyhow::Result<String> {
        let path = format!("/v0/projects/{}/reports", self.project.slug);
        let report = report(start, short, results)?;
        let json: serde_json::Value = self
            .api
            .send(Method::Post, &path, self.token, &report)
            .await?;
        json.get("uuid")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow::anyhow!("The report has no UUID: {json}"))
    }
}

/// Write each report's job, in one transaction beside the running API.
fn give_jobs(db: &Utf8Path, jobs: &[(&String, Job)]) -> anyhow::Result<()> {
    let mut conn = SqliteConnection::establish(db.as_str())?;
    diesel::sql_query("PRAGMA busy_timeout = 10000").execute(&mut conn)?;
    conn.transaction(|conn| {
        for (report, job) in jobs {
            let time = |at: Option<i64>| at.map_or_else(|| "NULL".to_owned(), |at| at.to_string());
            diesel::sql_query(format!(
                "INSERT INTO job (uuid, organization_id, project_id, report_id, source_ip, spec_id, \
                 config, timeout, priority, status, started, completed, created, modified) \
                 SELECT '{uuid}', project.organization_id, report.project_id, report.id, '127.0.0.1', \
                 (SELECT id FROM spec WHERE slug = '{SPEC}'), '{{}}', 3600, 0, {status}, {started}, \
                 {completed}, {created}, {created} \
                 FROM report JOIN project ON project.id = report.project_id WHERE report.uuid = '{report}'",
                uuid = JobUuid::new(),
                status = job.status.stored(),
                started = time(job.started),
                completed = time(job.completed),
                created = job.created,
            ))
            .execute(conn)?;
        }
        Ok::<_, diesel::result::Error>(())
    })?;
    Ok(())
}
