//! The data the console end-to-end suite runs against.
//!
//! Every report carries fixed start and end times, so a browser clock frozen at
//! [`NOW`] sees the same project on every run.

use anyhow::Context as _;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use bencher_json::{
    DateTime, Entitlements, JsonAuthUser, JsonOrganization, JsonProject, JsonProjectKeyCreated,
    JsonUser, JsonUserKeyCreated, Jwt, PlanLevel, Secret,
};
use bencher_license::Licensor;
use bencher_token::TokenKey;
use camino::Utf8Path;
use serde::Serialize;

use super::{ISSUER, SECRET_KEY, jobs};

// The test tokens are signed for these two emails.
const ADMIN: (&str, &str, &str) = (
    "Eustace Bagge",
    "eustace-bagge",
    "eustace.bagge@nowhere.com",
);
const MEMBER: (&str, &str, &str) = ("Muriel Bagge", "muriel-bagge", "muriel.bagge@nowhere.com");
/// Signed in, but a member of nothing: a reader of the public projects only.
const OUTSIDER: (&str, &str, &str) = ("Courage", "courage", "courage@nowhere.com");

const ORGANIZATION: &str = "Pompeii LLC";
const HASHBROWN: &str = "Hashbrown";
const VERSION_ZERO: &str = "Crinkle Cut";
const EMPTY: &str = "Tater Tot";
const PRIVATE: &str = "Home Fries";
const ITERATIONS: &str = "Waffle Fries";
const ADMIN_ORGANIZATION: &str = "Bagge Diner";
const ADMIN_NEWEST: &str = "Curly Fries";
const ADMIN_OLDER: &str = "Potato Wedges";

// Debug builds of the API trust this key's public half for self-hosted licenses.
const TEST_LICENSE_KEY: &str =
    include_str!("../../../../../plus/bencher_license/src/test/private.pem");
const TEST_LICENSE_ENTITLEMENTS: u32 = 1_000_000;

/// The time the browser clock is frozen at: 2026-09-14T00:00:00Z.
pub(super) const NOW: i64 = 1_789_344_000;
/// The newest report on `main`, which raises the one active alert: 2026-09-13T21:16:00Z.
const LAST_MAIN_REPORT: i64 = 1_789_334_160;
const MAIN_REPORTS: usize = 24;
/// Twenty four reports every 56 hours span a little under eight weeks.
const MAIN_INTERVAL: i64 = 56 * 60 * 60;
const FEATURE_REPORTS: usize = 3;
const FEATURE_INTERVAL: i64 = 20 * 60 * 60;
pub(super) const REPORT_DURATION: i64 = 2 * 60;

pub(super) const KIB_1: u32 = 1 << 10;
pub(super) const KIB_64: u32 = 1 << 16;

pub(super) const MAIN: &str = "main";
const FEATURE: &str = "feature-simd";
pub(super) const TESTBED: &str = "ubuntu-latest";

/// The project key that runs the newest report, on its own testbed so no threshold checks it.
const KEY: &str = "GitHub Actions";
const KEY_TESTBED: &str = "macos-latest";
const KEY_ADAPTER: &str = "magic";
/// Half an hour after the newest `main` report, on the same commit: 2026-09-13T21:46:00Z.
const KEY_REPORT: i64 = LAST_MAIN_REPORT + 30 * 60;
const KEY_DURATION: i64 = 4 * 60 + 12;
/// The short form of the newest `main` report's hash.
const LAST_MAIN_HASH: &str = "9c1f2e4";

/// A run split across reports, on `feature-simd` five weeks before the frozen
/// clock, so only windows longer than the default four weeks count it:
/// 2026-08-10T00:00:00Z.
const SPLIT_RUN: i64 = NOW - 5 * 7 * 24 * 60 * 60;
/// Each report's start, in seconds after the run's: three on [`TESTBED`] and one
/// on [`KEY_TESTBED`].
const SPLIT_STARTS: [(i64, &str); 4] = [(0, TESTBED), (2, TESTBED), (3, KEY_TESTBED), (4, TESTBED)];
/// The short form of the split run's hash.
const SPLIT_HASH: &str = "5917e00";
/// A report with no hash, a day after the split run.
const NO_HASH_REPORT: i64 = SPLIT_RUN + 24 * 60 * 60;

/// The variant whose latency jumps in the newest `main` report.
const ALERTING: Variant = Variant::new("blake3", KIB_64, Some(1), "avx2");
const ALERTING_LATENCY: f64 = 20.6;
/// The variant whose latency jumps in an older `main` report, an alert the seed dismisses.
const DISMISSED: Variant = Variant::new("sha256", KIB_64, Some(4), "avx2");
const DISMISSED_REPORT: usize = 15;
const DISMISSED_JUMP: f64 = 1.3;

/// A subset of the cross product: blake3 has every combination, the rest fewer.
const VARIANTS: [Variant; 18] = [
    Variant::new("blake3", KIB_1, Some(1), "avx2"),
    Variant::new("blake3", KIB_1, Some(1), "sse4.2"),
    Variant::new("blake3", KIB_1, Some(4), "avx2"),
    Variant::new("blake3", KIB_1, Some(4), "sse4.2"),
    ALERTING,
    Variant::new("blake3", KIB_64, Some(1), "sse4.2"),
    Variant::new("blake3", KIB_64, Some(4), "avx2"),
    Variant::new("blake3", KIB_64, Some(4), "sse4.2"),
    Variant::new("sha256", KIB_1, Some(1), "avx2"),
    Variant::new("sha256", KIB_1, Some(4), "avx2"),
    Variant::new("sha256", KIB_64, Some(1), "avx2"),
    DISMISSED,
    Variant::new("xxh3", KIB_1, None, "avx2"),
    Variant::new("xxh3", KIB_1, None, "sse4.2"),
    Variant::new("xxh3", KIB_64, None, "avx2"),
    Variant::new("xxh3", KIB_64, None, "sse4.2"),
    Variant::new("crc32c", KIB_1, None, "sse4.2"),
    Variant::new("crc32c", KIB_64, None, "sse4.2"),
];

/// Small run to run noise, so a threshold has a spread to compute a limit from.
const JITTER: [f64; 8] = [0.0, 0.004, -0.003, 0.002, -0.004, 0.003, -0.002, 0.001];

/// Runs of three iterations each, a day apart, the newest two hours before [`NOW`].
const ITERATION_REPORTS: usize = 6;
const ITERATION_INTERVAL: i64 = 24 * 60 * 60;
const LAST_ITERATION_REPORT: i64 = NOW - 2 * 60 * 60;
/// Each iteration's latency against its run's, so every run's range bar has a length.
const ITERATION_SPREAD: [f64; 3] = [0.96, 1.0, 1.05];
/// The newest run's second iteration jumps, which raises the project's one alert.
const ITERATION_JUMP: f64 = 1.3;

/// Version 0 runs a day apart, the newest three hours before [`NOW`], each
/// with a latency and its bounds and a throughput.
const V0_REPORTS: usize = 6;
const V0_INTERVAL: i64 = 24 * 60 * 60;
const LAST_V0_REPORT: i64 = NOW - 3 * 60 * 60;
/// The benchmark whose latency jumps in the newest version 0 run, which raises the project's one alert.
const V0_ALERTING: &str = "blake3";
/// Each version 0 benchmark with its steady latency.
const V0_BENCHMARKS: [(&str, f64); 2] = [(V0_ALERTING, 19.4), ("sha256", 49.6)];
const V0_JUMP: f64 = 1.3;
/// Each version 0 run's hash starts with its own seven digits, counting up from these.
const V0_HASH: u64 = 0x00c0_de00;
/// The spread of each latency's bounds around its value.
const V0_BOUND: f64 = 0.03;
/// The title of the version 0 project's one pin, made as the classic console makes one.
const V0_PIN: &str = "blake3, Latency on main";

/// The admin's two reports, a day apart, the newer an hour before [`NOW`].
const ADMIN_NEWEST_REPORT: i64 = NOW - 60 * 60;
const ADMIN_OLDER_REPORT: i64 = ADMIN_NEWEST_REPORT - 24 * 60 * 60;

/// What the suite reads: who to sign in as and which projects to open.
#[derive(Debug, Serialize)]
pub struct Seed {
    api_url: String,
    console_url: String,
    /// The API's log, which holds every email it sends.
    api_log: String,
    now: DateTime,
    admin: JsonAuthUser,
    member: JsonAuthUser,
    outsider: SeedReader,
    organization: SeedOrganization,
    projects: SeedProjects,
    /// The admin's version 1 project with the newest report, in an organization no test changes.
    admin_newest: SeedProject,
    last_main_hash: &'static str,
    split_hash: &'static str,
    /// The short form of the newest version 0 run's hash.
    version_zero_hash: String,
    jobs_hash: &'static str,
    held_hash: &'static str,
}

/// A signed in reader as the console stores one, holding a user key where
/// [`JsonAuthUser`] holds a token: only two emails have test tokens.
#[derive(Debug, Serialize)]
struct SeedReader {
    user: JsonUser,
    token: String,
    creation: DateTime,
    expiration: DateTime,
}

#[derive(Debug, Serialize)]
struct SeedOrganization {
    uuid: String,
    name: String,
    slug: String,
}

#[derive(Debug, Serialize)]
struct SeedProjects {
    /// Version 1 and public, with reports, thresholds, and one active alert.
    hashbrown: SeedProject,
    /// Still on version 0, with runs that send bounds, a bare threshold with one alert, and a
    /// classic pin.
    version_zero: SeedProject,
    /// Version 1 with no reports.
    empty: SeedProject,
    /// Version 1 and private.
    private: SeedProject,
    /// Version 1, with runs of three iterations on one line and one alert on a second iteration.
    iterations: SeedProject,
    /// Version 1, with runner jobs that wait, run, failed, and were canceled.
    runner: SeedProject,
    /// Version 1, each with one run that alerted and still runs, which the suite adds alerting
    /// reports to: one for each repeat, then one for the axe scan.
    held: Vec<SeedProject>,
    /// Version 1, each with one report, a queued job's, on an archived branch: one for each repeat.
    handover: Vec<SeedProject>,
}

#[derive(Debug, Serialize)]
struct SeedProject {
    uuid: String,
    name: String,
    slug: String,
}

impl Seed {
    pub async fn new(
        api_url: &str,
        console_url: &str,
        db: &Utf8Path,
        api_log: &str,
    ) -> anyhow::Result<Self> {
        let api = Api::new(api_url);

        // The first user to sign up is the server admin.
        let admin = api.sign_up(ADMIN, Jwt::test_admin_token()).await?;
        api.sign_up(MEMBER, Jwt::test_token()).await?;
        // Signed in with a session, not an API token, so the member can approve an app.
        let member = api.session(MEMBER.2).await?;
        let admin_token = admin.token.as_ref();
        let outsider = api.sign_up_with_key(OUTSIDER, admin_token).await?;
        let token = member.token.as_ref();

        let organization: JsonOrganization = api
            .send(
                Method::Post,
                "/v0/organizations",
                token,
                &serde_json::json!({ "name": ORGANIZATION }),
            )
            .await?;
        license(&api, &organization, token).await?;

        let hashbrown = api
            .project(&organization, HASHBROWN, "public", token)
            .await?;
        let version_zero = api
            .project(&organization, VERSION_ZERO, "public", token)
            .await?;
        let empty = api.project(&organization, EMPTY, "public", token).await?;
        let private = api
            .project(&organization, PRIVATE, "private", token)
            .await?;
        let iterations = api
            .project(&organization, ITERATIONS, "public", token)
            .await?;
        for project in [&hashbrown, &empty, &private, &iterations] {
            let path = format!("/v0/projects/{}", project.slug);
            let _project: JsonProject = api
                .send(
                    Method::Patch,
                    &path,
                    admin_token,
                    &serde_json::json!({ "bmf_version": 1 }),
                )
                .await?;
        }

        report_history(&api, &hashbrown, token).await?;
        key_report(&api, &hashbrown, token).await?;
        split_run(&api, &hashbrown, token).await?;
        check_alerts(&api, &hashbrown, token).await?;
        pins(&api, &hashbrown, token).await?;
        iteration_history(&api, &iterations, token).await?;
        version_zero_history(&api, &version_zero, token).await?;
        version_zero_pin(&api, &version_zero, token).await?;
        let jobs = jobs::seed(&api, db, &organization, token, admin_token).await?;
        let admin_newest = admin_projects(&api, admin_token).await?;

        Ok(Self {
            api_url: api_url.to_owned(),
            console_url: console_url.to_owned(),
            api_log: api_log.to_owned(),
            now: DateTime::try_from(NOW)?,
            admin,
            member,
            outsider,
            organization: SeedOrganization {
                uuid: organization.uuid.to_string(),
                name: organization.name.to_string(),
                slug: organization.slug.to_string(),
            },
            projects: SeedProjects {
                hashbrown: hashbrown.into(),
                version_zero: version_zero.into(),
                empty: empty.into(),
                private: private.into(),
                iterations: iterations.into(),
                runner: jobs.runner.into(),
                held: jobs.held.into_iter().map(Into::into).collect(),
                handover: jobs.handover.into_iter().map(Into::into).collect(),
            },
            admin_newest,
            last_main_hash: LAST_MAIN_HASH,
            split_hash: SPLIT_HASH,
            version_zero_hash: format!("{:07x}", V0_HASH + u64::try_from(V0_REPORTS - 1)?),
            jobs_hash: jobs::JOBS_HASH,
            held_hash: jobs::HELD_HASH,
        })
    }
}

impl From<JsonProject> for SeedProject {
    fn from(project: JsonProject) -> Self {
        Self {
            uuid: project.uuid.to_string(),
            name: project.name.to_string(),
            slug: project.slug.to_string(),
        }
    }
}

/// Two projects of the admin's, the newer report posted first so the newest is not
/// the last posted.
async fn admin_projects(api: &Api, token: &str) -> anyhow::Result<SeedProject> {
    let organization: JsonOrganization = api
        .send(
            Method::Post,
            "/v0/organizations",
            token,
            &serde_json::json!({ "name": ADMIN_ORGANIZATION }),
        )
        .await?;
    let newest =
        admin_project(api, &organization, ADMIN_NEWEST, ADMIN_NEWEST_REPORT, token).await?;
    admin_project(api, &organization, ADMIN_OLDER, ADMIN_OLDER_REPORT, token).await?;
    Ok(newest)
}

/// A version 1 project with one report that starts at `start`.
async fn admin_project(
    api: &Api,
    organization: &JsonOrganization,
    name: &str,
    start: i64,
    token: &str,
) -> anyhow::Result<SeedProject> {
    let project = api.project(organization, name, "public", token).await?;
    let path = format!("/v0/projects/{}", project.slug);
    let project: JsonProject = api
        .send(
            Method::Patch,
            &path,
            token,
            &serde_json::json!({ "bmf_version": 1 }),
        )
        .await?;
    let report = report_body(MAIN, start, &main_hash(0), &results(0, |_| 1.0))?;
    let _json: serde_json::Value = api
        .send(Method::Post, &format!("{path}/reports"), token, &report)
        .await?;
    Ok(project.into())
}

/// A private project needs a plan, so the organization gets a self-hosted license.
async fn license(api: &Api, organization: &JsonOrganization, token: &str) -> anyhow::Result<()> {
    let key: Secret = TEST_LICENSE_KEY.parse()?;
    let license = Licensor::bencher_cloud(&key)?.new_annual_license(
        organization.uuid,
        PlanLevel::Enterprise,
        Entitlements::try_from(TEST_LICENSE_ENTITLEMENTS)?,
    )?;
    let path = format!("/v0/organizations/{}", organization.slug);
    let _organization: JsonOrganization = api
        .send(
            Method::Patch,
            &path,
            token,
            &serde_json::json!({ "license": license }),
        )
        .await?;
    Ok(())
}

/// Eight weeks of `main` and a few days of `feature-simd`, with a latency threshold on `main`.
async fn report_history(api: &Api, project: &JsonProject, token: &str) -> anyhow::Result<()> {
    let path = format!("/v0/projects/{}/reports", project.slug);
    let mut last = 0;
    for index in 0..MAIN_REPORTS {
        let start = LAST_MAIN_REPORT - MAIN_INTERVAL * i64::try_from(MAIN_REPORTS - 1 - index)?;
        let mut report = report_body(MAIN, start, &main_hash(index), &main_results(index))?;
        // A run declares its thresholds, so the first run creates the one on Latency.
        if index == 0
            && let Some(object) = report.as_object_mut()
        {
            object.insert("thresholds".to_owned(), latency_threshold());
        }
        let json: serde_json::Value = api.send(Method::Post, &path, token, &report).await?;
        if index == DISMISSED_REPORT {
            dismiss_alerts(api, project, &json, token).await?;
        }
        last = start;
    }
    for index in 0..FEATURE_REPORTS {
        let start = last - FEATURE_INTERVAL * i64::try_from(FEATURE_REPORTS - index)?;
        let hash = format!("{:040x}", 0xfeed_0000u64 + u64::try_from(index)?);
        let report = report_body(FEATURE, start, &hash, &results(index, |_| 1.0))?;
        let _json: serde_json::Value = api.send(Method::Post, &path, token, &report).await?;
    }
    Ok(())
}

/// A run split across testbeds, then a report that names no commit.
async fn split_run(api: &Api, project: &JsonProject, token: &str) -> anyhow::Result<()> {
    let path = format!("/v0/projects/{}/reports", project.slug);
    let hash = format!("{SPLIT_HASH}{}", "0".repeat(33));
    for (index, (offset, testbed)) in SPLIT_STARTS.into_iter().enumerate() {
        let mut report = report_body(FEATURE, SPLIT_RUN + offset, &hash, &results(index, |_| 1.0))?;
        if let Some(object) = report.as_object_mut() {
            object.insert("testbed".to_owned(), testbed.into());
        }
        let _json: serde_json::Value = api.send(Method::Post, &path, token, &report).await?;
    }
    let mut report = report_body(FEATURE, NO_HASH_REPORT, "", &results(0, |_| 1.0))?;
    if let Some(object) = report.as_object_mut() {
        object.remove("hash");
    }
    let _json: serde_json::Value = api.send(Method::Post, &path, token, &report).await?;
    Ok(())
}

fn report_body(
    branch: &str,
    start: i64,
    hash: &str,
    results: &str,
) -> anyhow::Result<serde_json::Value> {
    Ok(serde_json::json!({
        "branch": branch,
        "hash": hash,
        "testbed": TESTBED,
        "start_time": DateTime::try_from(start)?,
        "end_time": DateTime::try_from(start + REPORT_DURATION)?,
        "results": [results],
        "settings": { "adapter": "json" },
    }))
}

/// A run by a project key rather than a person: two variants with both measures, four lines.
async fn key_report(api: &Api, project: &JsonProject, token: &str) -> anyhow::Result<()> {
    let path = format!("/v0/projects/{}/keys", project.slug);
    let key: JsonProjectKeyCreated = api
        .send(
            Method::Post,
            &path,
            token,
            &serde_json::json!({ "name": KEY }),
        )
        .await?;
    let results = serde_json::json!({
        "blake3": [{
            "parameters": ALERTING.parameters(),
            "measures": {
                "latency": { "value": 15.2 },
                "throughput": { "value": 4311.6 },
            },
        }],
        "xxh3": [{
            "parameters": { "input_bytes": KIB_64, "simd": "avx2" },
            "measures": {
                "latency": { "value": 2.9 },
                "throughput": { "value": 22_598.6 },
            },
        }],
    });
    let report = serde_json::json!({
        "branch": MAIN,
        "hash": main_hash(MAIN_REPORTS - 1),
        "testbed": KEY_TESTBED,
        "start_time": DateTime::try_from(KEY_REPORT)?,
        "end_time": DateTime::try_from(KEY_REPORT + KEY_DURATION)?,
        "results": [results.to_string()],
        "settings": { "adapter": KEY_ADAPTER },
    });
    let path = format!("/v0/projects/{}/reports", project.slug);
    let _json: serde_json::Value = api
        .send(Method::Post, &path, key.key.as_ref(), &report)
        .await?;
    Ok(())
}

fn latency_threshold() -> serde_json::Value {
    serde_json::json!({
        "models": [{
            "measure": "latency",
            "metric": "value",
            "model": {
                "test": "t_test",
                "min_sample_size": 4,
                "max_sample_size": 64,
                "upper_boundary": 0.99,
            },
        }]
    })
}

/// The newest hash starts with [`LAST_MAIN_HASH`]; the rest only have to differ.
fn main_hash(index: usize) -> String {
    if index == MAIN_REPORTS - 1 {
        format!("{LAST_MAIN_HASH}{:033x}", 0u8)
    } else {
        format!(
            "{:040x}",
            0xabcd_0000u64 + u64::try_from(index).unwrap_or_default()
        )
    }
}

fn main_results(index: usize) -> String {
    results(index, |variant| {
        if index == DISMISSED_REPORT && variant == &DISMISSED {
            DISMISSED_JUMP
        } else {
            1.0
        }
    })
}

/// Runs of blake3 at 64 KiB, three iterations each, with a percentage threshold
/// on latency that the newest run's second iteration crosses.
async fn iteration_history(api: &Api, project: &JsonProject, token: &str) -> anyhow::Result<()> {
    let path = format!("/v0/projects/{}/reports", project.slug);
    for index in 0..ITERATION_REPORTS {
        let start = LAST_ITERATION_REPORT
            - ITERATION_INTERVAL * i64::try_from(ITERATION_REPORTS - 1 - index)?;
        let noise = JITTER.get(index).copied().unwrap_or_default();
        let results = ITERATION_SPREAD
            .iter()
            .enumerate()
            .map(|(iteration, spread)| {
                let jump = if index == ITERATION_REPORTS - 1 && iteration == 1 {
                    ITERATION_JUMP
                } else {
                    1.0
                };
                let latency = ALERTING.latency() * spread * (1.0 + noise) * jump;
                serde_json::json!({
                    ALERTING.benchmark: [{
                        "parameters": { "input_bytes": KIB_64 },
                        "measures": {
                            "latency": { "value": latency },
                            "throughput": { "value": f64::from(KIB_64) / latency },
                        },
                    }],
                })
                .to_string()
            })
            .collect::<Vec<_>>();
        let mut report = serde_json::json!({
            "branch": MAIN,
            "hash": format!("{:040x}", 0x17e0_0000u64 + u64::try_from(index)?),
            "testbed": TESTBED,
            "start_time": DateTime::try_from(start)?,
            "end_time": DateTime::try_from(start + REPORT_DURATION)?,
            "results": results,
            "settings": { "adapter": "json" },
        });
        if index == 0
            && let Some(object) = report.as_object_mut()
        {
            object.insert(
                "thresholds".to_owned(),
                serde_json::json!({
                    "models": [{
                        "measure": "latency",
                        "metric": "value",
                        "model": { "test": "percentage", "upper_boundary": 0.1 },
                    }]
                }),
            );
        }
        let _json: serde_json::Value = api.send(Method::Post, &path, token, &report).await?;
    }
    let path = format!("/v0/projects/{}/alerts", project.slug);
    let alerts: Vec<serde_json::Value> = api.get(&path, token).await?;
    anyhow::ensure!(
        alerts.len() == 1,
        "Expected one alert in {ITERATIONS}, found {}",
        alerts.len()
    );
    Ok(())
}

/// A version 1 payload: every variant with its latency and throughput.
fn results<F>(index: usize, jump: F) -> String
where
    F: Fn(&Variant) -> f64,
{
    let mut benchmarks = serde_json::Map::new();
    for (offset, variant) in VARIANTS.iter().enumerate() {
        let noise = JITTER
            .iter()
            .cycle()
            .nth(index + offset)
            .copied()
            .unwrap_or_default();
        let latency = if index == MAIN_REPORTS - 1 && variant == &ALERTING {
            ALERTING_LATENCY
        } else {
            variant.latency() * (1.0 + noise) * jump(variant)
        };
        let entry = serde_json::json!({
            "parameters": variant.parameters(),
            "measures": {
                "latency": { "value": latency },
                "throughput": { "value": f64::from(variant.input_bytes) / latency },
            },
        });
        if let serde_json::Value::Array(entries) = benchmarks
            .entry(variant.benchmark)
            .or_insert_with(|| serde_json::Value::Array(Vec::new()))
        {
            entries.push(entry);
        }
    }
    serde_json::Value::Object(benchmarks).to_string()
}

async fn dismiss_alerts(
    api: &Api,
    project: &JsonProject,
    report: &serde_json::Value,
    token: &str,
) -> anyhow::Result<()> {
    let alerts = report
        .get("alerts")
        .and_then(serde_json::Value::as_array)
        .context("The report lists no alerts")?;
    anyhow::ensure!(
        alerts.len() == 1,
        "Expected one alert to dismiss, found {}",
        alerts.len()
    );
    for alert in alerts {
        let uuid = alert
            .get("uuid")
            .and_then(serde_json::Value::as_str)
            .context("An alert has no UUID")?;
        let path = format!("/v0/projects/{}/alerts/{uuid}", project.slug);
        let _alert: serde_json::Value = api
            .send(
                Method::Patch,
                &path,
                token,
                &serde_json::json!({ "status": "dismissed" }),
            )
            .await?;
    }
    Ok(())
}

/// The suite counts on exactly one active alert, on the variant that jumped last.
async fn check_alerts(api: &Api, project: &JsonProject, token: &str) -> anyhow::Result<()> {
    let path = format!("/v0/projects/{}/alerts?status=active", project.slug);
    let alerts: Vec<serde_json::Value> = api.get(&path, token).await?;
    let [alert] = alerts.as_slice() else {
        anyhow::bail!("Expected one active alert, found {}", alerts.len());
    };
    let benchmark = alert
        .pointer("/benchmark/name")
        .and_then(serde_json::Value::as_str);
    let parameters = alert.pointer("/variant/parameters");
    anyhow::ensure!(
        benchmark == Some(ALERTING.benchmark) && parameters == Some(&ALERTING.parameters()),
        "The active alert is on {alert}"
    );
    let path = format!("/v0/projects/{}/alerts", project.slug);
    let all: Vec<serde_json::Value> = api.get(&path, token).await?;
    anyhow::ensure!(
        all.len() == 2,
        "Expected one dismissed alert beside it, found {}",
        all.len()
    );
    Ok(())
}

/// The UUID of the project's dimension whose `field` is `value`.
async fn dimension_uuid(
    api: &Api,
    project: &JsonProject,
    token: &str,
    (dimension, field, value): (&str, &str, &str),
) -> anyhow::Result<String> {
    let path = format!("/v0/projects/{}/{dimension}?per_page=64", project.slug);
    let list: Vec<serde_json::Value> = api.get(&path, token).await?;
    list.iter()
        .find(|entry| entry.get(field).and_then(serde_json::Value::as_str) == Some(value))
        .and_then(|entry| entry.get("uuid").and_then(serde_json::Value::as_str))
        .map(ToOwned::to_owned)
        .with_context(|| format!("No {dimension} with {field} {value}"))
}

/// Three pins on Plots, top first, each at its own window: the first holds the active alert.
async fn pins(api: &Api, project: &JsonProject, token: &str) -> anyhow::Result<()> {
    let uuid = async |dimension: &str, field: &str, value: &str| {
        dimension_uuid(api, project, token, (dimension, field, value)).await
    };
    let branch = uuid("branches", "name", MAIN).await?;
    let testbed = uuid("testbeds", "name", TESTBED).await?;
    let latency = uuid("measures", "slug", "latency").await?;
    let throughput = uuid("measures", "slug", "throughput").await?;
    let path = format!("/v0/projects/{}/plots", project.slug);
    // Each pin goes to the top, so the last pinned is first.
    for (title, benchmark, parameters, measures, days) in [
        (
            "crc32c, Latency and Throughput on main",
            "crc32c",
            None,
            vec![latency.clone(), throughput],
            92,
        ),
        (
            "xxh3, Latency on main",
            "xxh3",
            None,
            vec![latency.clone()],
            7,
        ),
        (
            "blake3 input_bytes=65536 threads=1, Latency on main",
            ALERTING.benchmark,
            Some(serde_json::json!([{ "input_bytes": KIB_64, "threads": 1 }])),
            vec![latency.clone()],
            28,
        ),
    ] {
        let mut body = serde_json::json!({
            "title": title,
            "lower_value": false,
            "upper_value": false,
            "lower_boundary": false,
            "upper_boundary": false,
            "x_axis": "date_time",
            "layout": "dual",
            "window": days * 24 * 60 * 60,
            "branches": [branch],
            "testbeds": [testbed],
            "benchmarks": [uuid("benchmarks", "name", benchmark).await?],
            "measures": measures,
            "metrics": ["value"],
        });
        if let (Some(parameters), Some(object)) = (parameters, body.as_object_mut()) {
            object.insert("parameters".to_owned(), parameters);
        }
        let _plot: serde_json::Value = api.send(Method::Post, &path, token, &body).await?;
    }
    Ok(())
}

/// Version 0 runs whose first declares a bare percentage threshold on latency,
/// which the newest run's jump on blake3 crosses.
async fn version_zero_history(api: &Api, project: &JsonProject, token: &str) -> anyhow::Result<()> {
    let path = format!("/v0/projects/{}/reports", project.slug);
    for index in 0..V0_REPORTS {
        let start = LAST_V0_REPORT - V0_INTERVAL * i64::try_from(V0_REPORTS - 1 - index)?;
        let noise = JITTER.get(index).copied().unwrap_or_default();
        let mut benchmarks = serde_json::Map::new();
        for (benchmark, steady) in V0_BENCHMARKS {
            let jump = if index == V0_REPORTS - 1 && benchmark == V0_ALERTING {
                V0_JUMP
            } else {
                1.0
            };
            let latency = steady * (1.0 + noise) * jump;
            benchmarks.insert(
                benchmark.to_owned(),
                serde_json::json!({
                    "latency": {
                        "value": latency,
                        "lower_value": latency * (1.0 - V0_BOUND),
                        "upper_value": latency * (1.0 + V0_BOUND),
                    },
                    "throughput": { "value": f64::from(KIB_64) / latency },
                }),
            );
        }
        let hash = format!("{:07x}{}", V0_HASH + u64::try_from(index)?, "0".repeat(33));
        let mut report = report_body(
            MAIN,
            start,
            &hash,
            &serde_json::Value::Object(benchmarks).to_string(),
        )?;
        if index == 0
            && let Some(object) = report.as_object_mut()
        {
            object.insert(
                "thresholds".to_owned(),
                serde_json::json!({
                    "models": { "latency": { "test": "percentage", "upper_boundary": 0.1 } }
                }),
            );
        }
        let _json: serde_json::Value = api.send(Method::Post, &path, token, &report).await?;
    }
    let path = format!("/v0/projects/{}/alerts", project.slug);
    let alerts: Vec<serde_json::Value> = api.get(&path, token).await?;
    anyhow::ensure!(
        alerts.len() == 1,
        "Expected one alert in {VERSION_ZERO}, found {}",
        alerts.len()
    );
    Ok(())
}

/// A pin as the classic console makes one, with no metrics and its lower bound shown.
async fn version_zero_pin(api: &Api, project: &JsonProject, token: &str) -> anyhow::Result<()> {
    let uuid = async |dimension: &str, field: &str, value: &str| {
        dimension_uuid(api, project, token, (dimension, field, value)).await
    };
    let body = serde_json::json!({
        "title": V0_PIN,
        "lower_value": true,
        "upper_value": false,
        "lower_boundary": false,
        "upper_boundary": true,
        "x_axis": "date_time",
        "window": 28 * 24 * 60 * 60,
        "branches": [uuid("branches", "name", MAIN).await?],
        "testbeds": [uuid("testbeds", "name", TESTBED).await?],
        "benchmarks": [uuid("benchmarks", "name", V0_ALERTING).await?],
        "measures": [uuid("measures", "slug", "latency").await?],
    });
    let path = format!("/v0/projects/{}/plots", project.slug);
    let _plot: serde_json::Value = api.send(Method::Post, &path, token, &body).await?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Variant {
    benchmark: &'static str,
    input_bytes: u32,
    threads: Option<u32>,
    simd: &'static str,
}

impl Variant {
    const fn new(
        benchmark: &'static str,
        input_bytes: u32,
        threads: Option<u32>,
        simd: &'static str,
    ) -> Self {
        Self {
            benchmark,
            input_bytes,
            threads,
            simd,
        }
    }

    fn parameters(&self) -> serde_json::Value {
        let mut parameters = serde_json::json!({
            "input_bytes": self.input_bytes,
            "simd": self.simd,
        });
        if let (Some(threads), Some(object)) = (self.threads, parameters.as_object_mut()) {
            object.insert("threads".to_owned(), threads.into());
        }
        parameters
    }

    /// The steady latency in nanoseconds; blake3 at 64 KiB on one thread with AVX2 is 19.4.
    fn latency(&self) -> f64 {
        let benchmark = match self.benchmark {
            "blake3" => 1.2125,
            "sha256" => 3.1,
            "xxh3" => 0.21,
            _ => 0.48,
        };
        let size = if self.input_bytes == KIB_1 { 1.0 } else { 16.0 };
        let threads = if self.threads == Some(4) { 0.55 } else { 1.0 };
        let simd = if self.simd == "avx2" { 1.0 } else { 1.35 };
        benchmark * size * threads * simd
    }
}

/// A thin JSON client for the API under test.
pub(super) struct Api {
    client: reqwest::Client,
    url: String,
}

#[derive(Clone, Copy)]
pub(super) enum Method {
    Post,
    Patch,
}

impl Api {
    fn new(url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: url.to_owned(),
        }
    }

    /// Sign a user up and return them as the console stores a signed in user.
    async fn sign_up(
        &self,
        (name, slug, email): (&str, &str, &str),
        token: Jwt,
    ) -> anyhow::Result<JsonAuthUser> {
        let body =
            serde_json::json!({ "name": name, "slug": slug, "email": email, "i_agree": true });
        let _ack: serde_json::Value = self
            .send(Method::Post, "/v0/auth/signup", "", &body)
            .await?;
        let user: JsonUser = self
            .get(&format!("/v0/users/{slug}"), token.as_ref())
            .await?;
        let Claims { iat, exp } = Claims::decode(&token)?;
        Ok(JsonAuthUser {
            user,
            token,
            creation: DateTime::try_from(iat)?,
            expiration: DateTime::try_from(exp)?,
        })
    }

    /// A sign in session for `email`, as its emailed link mints one.
    async fn session(&self, email: &str) -> anyhow::Result<JsonAuthUser> {
        let token = TokenKey::new(ISSUER.to_owned(), &SECRET_KEY.parse::<Secret>()?)
            .new_auth(email.parse()?, 60)?;
        self.send(
            Method::Post,
            "/v0/auth/confirm",
            "",
            &serde_json::json!({ "token": token }),
        )
        .await
    }

    /// Sign a user up and sign them in with a user key the admin makes for them.
    async fn sign_up_with_key(
        &self,
        (name, slug, email): (&str, &str, &str),
        admin_token: &str,
    ) -> anyhow::Result<SeedReader> {
        let body =
            serde_json::json!({ "name": name, "slug": slug, "email": email, "i_agree": true });
        let _ack: serde_json::Value = self
            .send(Method::Post, "/v0/auth/signup", "", &body)
            .await?;
        let user: JsonUser = self.get(&format!("/v0/users/{slug}"), admin_token).await?;
        let key: JsonUserKeyCreated = self
            .send(
                Method::Post,
                &format!("/v0/users/{slug}/keys"),
                admin_token,
                &serde_json::json!({ "name": "console" }),
            )
            .await?;
        Ok(SeedReader {
            user,
            token: key.key.into(),
            creation: key.creation,
            expiration: key.expiration,
        })
    }

    pub(super) async fn project(
        &self,
        organization: &JsonOrganization,
        name: &str,
        visibility: &str,
        token: &str,
    ) -> anyhow::Result<JsonProject> {
        let path = format!("/v0/organizations/{}/projects", organization.slug);
        let body = serde_json::json!({ "name": name, "visibility": visibility });
        self.send(Method::Post, &path, token, &body).await
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        token: &str,
    ) -> anyhow::Result<T> {
        let request = self
            .client
            .get(format!("{}{path}", self.url))
            .bearer_auth(token);
        read(request, path).await
    }

    pub(super) async fn send<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        token: &str,
        body: &serde_json::Value,
    ) -> anyhow::Result<T> {
        let url = format!("{}{path}", self.url);
        let request = match method {
            Method::Post => self.client.post(url),
            Method::Patch => self.client.patch(url),
        };
        let request = if token.is_empty() {
            request
        } else {
            request.bearer_auth(token)
        };
        read(request.json(body), path).await
    }
}

async fn read<T: serde::de::DeserializeOwned>(
    request: reqwest::RequestBuilder,
    path: &str,
) -> anyhow::Result<T> {
    let response = request.send().await?;
    let status = response.status();
    let text = response.text().await?;
    anyhow::ensure!(status.is_success(), "{path} answered {status}: {text}");
    serde_json::from_str(&text).with_context(|| format!("{path} answered {text}"))
}

/// The issue and expiry times a token carries.
#[derive(serde::Deserialize)]
struct Claims {
    iat: i64,
    exp: i64,
}

impl Claims {
    fn decode(token: &Jwt) -> anyhow::Result<Self> {
        let payload = token
            .as_ref()
            .split('.')
            .nth(1)
            .context("The token has no claims")?;
        let bytes = URL_SAFE_NO_PAD.decode(payload)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}
