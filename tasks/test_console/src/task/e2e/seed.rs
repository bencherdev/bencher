//! The data the console end-to-end suite runs against.
//!
//! Every report carries fixed start and end times, so a browser clock frozen at
//! [`NOW`] sees the same project on every run.

use anyhow::Context as _;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use bencher_json::{
    DateTime, Entitlements, JsonAuthUser, JsonOrganization, JsonProject, JsonUser, Jwt, PlanLevel,
    Secret,
};
use bencher_license::Licensor;
use serde::Serialize;

// The test tokens are signed for these two emails.
const ADMIN: (&str, &str, &str) = (
    "Eustace Bagge",
    "eustace-bagge",
    "eustace.bagge@nowhere.com",
);
const MEMBER: (&str, &str, &str) = ("Muriel Bagge", "muriel-bagge", "muriel.bagge@nowhere.com");

const ORGANIZATION: &str = "Pompeii LLC";
const HASHBROWN: &str = "Hashbrown";
const VERSION_ZERO: &str = "Crinkle Cut";
const EMPTY: &str = "Tater Tot";
const PRIVATE: &str = "Home Fries";

// Debug builds of the API trust this key's public half for self-hosted licenses.
const TEST_LICENSE_KEY: &str =
    include_str!("../../../../../plus/bencher_license/src/test/private.pem");
const TEST_LICENSE_ENTITLEMENTS: u32 = 1_000_000;

/// The time the browser clock is frozen at: 2026-09-14T00:00:00Z.
const NOW: i64 = 1_789_344_000;
/// The newest report on `main`, which raises the one active alert: 2026-09-13T21:16:00Z.
const LAST_MAIN_REPORT: i64 = 1_789_334_160;
const MAIN_REPORTS: usize = 24;
/// Twenty four reports every 56 hours span a little under eight weeks.
const MAIN_INTERVAL: i64 = 56 * 60 * 60;
const FEATURE_REPORTS: usize = 3;
const FEATURE_INTERVAL: i64 = 20 * 60 * 60;
const REPORT_DURATION: i64 = 2 * 60;

const KIB_1: u32 = 1 << 10;
const KIB_64: u32 = 1 << 16;

const MAIN: &str = "main";
const FEATURE: &str = "feature-simd";
const TESTBED: &str = "ubuntu-latest";
/// The short form of the newest `main` report's hash.
const LAST_MAIN_HASH: &str = "9c1f2e4";

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

/// What the suite reads: who to sign in as and which projects to open.
#[derive(Debug, Serialize)]
pub struct Seed {
    api_url: String,
    console_url: String,
    now: DateTime,
    admin: JsonAuthUser,
    member: JsonAuthUser,
    organization: SeedOrganization,
    projects: SeedProjects,
    last_main_hash: &'static str,
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
    /// Still on version 0, so it belongs to the classic console.
    version_zero: SeedProject,
    /// Version 1 with no reports.
    empty: SeedProject,
    /// Version 1 and private.
    private: SeedProject,
}

#[derive(Debug, Serialize)]
struct SeedProject {
    uuid: String,
    name: String,
    slug: String,
}

impl Seed {
    pub async fn new(api_url: &str, console_url: &str) -> anyhow::Result<Self> {
        let api = Api::new(api_url);

        // The first user to sign up is the server admin.
        let admin = api.sign_up(ADMIN, Jwt::test_admin_token()).await?;
        let member = api.sign_up(MEMBER, Jwt::test_token()).await?;
        let admin_token = admin.token.as_ref();
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
        for project in [&hashbrown, &empty, &private] {
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
        check_alerts(&api, &hashbrown, token).await?;

        Ok(Self {
            api_url: api_url.to_owned(),
            console_url: console_url.to_owned(),
            now: DateTime::try_from(NOW)?,
            admin,
            member,
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
            },
            last_main_hash: LAST_MAIN_HASH,
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
struct Api {
    client: reqwest::Client,
    url: String,
}

#[derive(Clone, Copy)]
enum Method {
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

    async fn project(
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

    async fn send<T: serde::de::DeserializeOwned>(
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
