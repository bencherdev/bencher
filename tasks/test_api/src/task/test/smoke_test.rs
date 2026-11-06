use std::sync::LazyLock;
use std::{
    collections::BTreeSet,
    net::TcpStream,
    process::{Child, Command},
    thread,
    time::Duration,
};

use assert_cmd::{assert::OutputAssertExt as _, cargo::CommandCargoExt as _};
use bencher_json::{
    BENCHER_API_PORT, BmfVersion, DEV_BENCHER_API_URL, DEV_BENCHER_REGISTRY_URL, JsonApiVersion,
    JsonProject, JsonReport, JsonReportIterationCounts, Jwt, LOCALHOST_BENCHER_API_URL,
    LOCALHOST_BENCHER_REGISTRY_URL, PROD_BENCHER_API_URL, PROD_BENCHER_REGISTRY_URL,
    TEST_BENCHER_API_URL, TEST_BENCHER_REGISTRY_URL, Url, project::report::ParseOutcome,
};

use crate::{
    API_VERSION,
    parser::{TaskExamples, TaskSeedTest, TaskSmokeTest, TaskTestEnvironment},
    task::test::{
        examples::Examples,
        seed_test::{
            BENCHER_CMD, BRANCH_ARG, BRANCH_SLUG, CLI_DIR, HOST_ARG, PROJECT_ARG, PROJECT_SLUG,
            SeedTest, TESTBED_ARG, TESTBED_SLUG, TOKEN_ARG,
        },
    },
};
#[cfg(feature = "plus")]
use crate::{
    parser::{TaskOci, TaskRunner},
    task::plus::{oci::Oci, runner},
};

const DEV_ADMIN_BENCHER_API_TOKEN_STR: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJjbGllbnQiLCJleHAiOjYwNjAwMjk0MTQsImlhdCI6MTc2NTA2MjExOSwiaXNzIjoiaHR0cHM6Ly9kZXZlbC0tYmVuY2hlci5uZXRsaWZ5LmFwcC8iLCJzdWIiOiJldXN0YWNlLmJhZ2dlQG5vd2hlcmUuY29tIiwib3JnIjpudWxsLCJzdGF0ZSI6bnVsbH0.0OXSTC4glSFFX5SIKUOg4mgD3z5bd2YyhE0rhMN7eRI";
const DEV_BENCHER_API_TOKEN_STR: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJjbGllbnQiLCJleHAiOjU5OTM2NDIxNTYsImlhdCI6MTY5ODY3NDg2MSwiaXNzIjoiaHR0cHM6Ly9kZXZlbC0tYmVuY2hlci5uZXRsaWZ5LmFwcC8iLCJzdWIiOiJtdXJpZWwuYmFnZ2VAbm93aGVyZS5jb20iLCJvcmciOm51bGx9.-JVsL3aj610rm2O29ZN1UzHtVQL116hE5JtJfj55E5M";

pub(crate) static DEV_ADMIN_BENCHER_API_TOKEN: LazyLock<Jwt> = LazyLock::new(|| {
    DEV_ADMIN_BENCHER_API_TOKEN_STR
        .parse()
        .expect("Invalid test JWT")
});

pub(crate) static DEV_BENCHER_API_TOKEN: LazyLock<Jwt> =
    LazyLock::new(|| DEV_BENCHER_API_TOKEN_STR.parse().expect("Invalid test JWT"));

#[derive(Debug)]
pub struct SmokeTest {
    pub environment: Environment,
}

#[derive(Debug, Clone, Copy)]
pub enum Environment {
    Ci,
    Localhost,
    Docker,
    Dev,
    Test,
    Prod,
}

impl TryFrom<TaskSmokeTest> for SmokeTest {
    type Error = anyhow::Error;

    fn try_from(test: TaskSmokeTest) -> Result<Self, Self::Error> {
        let TaskSmokeTest { environment } = test;
        Ok(Self {
            environment: environment.unwrap_or_default().into(),
        })
    }
}

impl From<TaskTestEnvironment> for Environment {
    fn from(environment: TaskTestEnvironment) -> Self {
        match environment {
            TaskTestEnvironment::Ci => Self::Ci,
            TaskTestEnvironment::Localhost => Self::Localhost,
            TaskTestEnvironment::Docker => Self::Docker,
            TaskTestEnvironment::Dev => Self::Dev,
            TaskTestEnvironment::Test => Self::Test,
            TaskTestEnvironment::Prod => Self::Prod,
        }
    }
}

impl SmokeTest {
    pub fn exec(&self) -> anyhow::Result<()> {
        let child = match self.environment {
            Environment::Ci | Environment::Localhost => Some(api_run()?),
            Environment::Docker => bencher_up().map(|()| None)?,
            Environment::Dev | Environment::Test | Environment::Prod => None,
        };

        let api_url = self.environment.as_url();
        let registry_url = self.environment.as_registry_url();
        test_api_version(&api_url)?;

        match self.environment {
            Environment::Ci => {
                test(
                    &api_url,
                    &registry_url,
                    MockSetup::SelfHosted {
                        examples: false,
                        runner: true,
                    },
                )?;
                kill_child(child)?;
            },
            Environment::Localhost => {
                test(
                    &api_url,
                    &registry_url,
                    MockSetup::SelfHosted {
                        examples: true,
                        runner: false,
                    },
                )?;
                kill_child(child)?;
            },
            Environment::Docker => bencher_down()?,
            Environment::Dev => test(&api_url, &registry_url, MockSetup::BencherCloud)?,
            Environment::Test | Environment::Prod => {},
        }

        Ok(())
    }
}

impl Environment {
    fn as_url(self) -> Url {
        match self {
            Self::Ci | Self::Localhost | Self::Docker => LOCALHOST_BENCHER_API_URL.clone(),
            Self::Dev => DEV_BENCHER_API_URL.clone(),
            Self::Test => TEST_BENCHER_API_URL.clone(),
            Self::Prod => PROD_BENCHER_API_URL.clone(),
        }
        .into()
    }

    fn as_registry_url(self) -> Url {
        match self {
            Self::Ci | Self::Localhost | Self::Docker => LOCALHOST_BENCHER_REGISTRY_URL.clone(),
            Self::Dev => DEV_BENCHER_REGISTRY_URL.clone(),
            Self::Test => TEST_BENCHER_REGISTRY_URL.clone(),
            Self::Prod => PROD_BENCHER_REGISTRY_URL.clone(),
        }
        .into()
    }
}

fn api_run() -> anyhow::Result<Child> {
    let mut child = Command::new("cargo")
        .args(["run"])
        .current_dir("./services/api")
        .spawn()?;

    while TcpStream::connect(("localhost", BENCHER_API_PORT)).is_err() {
        if let Some(status) = child.try_wait()? {
            anyhow::bail!(
                "API server process exited before it started listening. Exit status: {status}"
            );
        }
        thread::sleep(Duration::from_secs(1));
        println!("Waiting for API server to start...");
    }

    Ok(child)
}

#[expect(clippy::panic_in_result_fn, reason = "test harness")]
fn bencher_up() -> anyhow::Result<()> {
    // Use the `latest`` image tag so this test doesn't fail when releasing a new version.
    let status = Command::new("cargo")
        .args(["run", "--", "up", "--detach", "--tag", "latest", "api"])
        .current_dir("./services/cli")
        .status()?;
    assert!(status.success(), "{status}");

    while TcpStream::connect(("localhost", BENCHER_API_PORT)).is_err() {
        thread::sleep(Duration::from_secs(1));
        println!("Waiting for API server to start...");
    }

    Ok(())
}

#[expect(clippy::panic_in_result_fn, reason = "test harness")]
fn bencher_down() -> anyhow::Result<()> {
    let status = Command::new("cargo")
        .args(["run", "--", "down", "api"])
        .current_dir("./services/cli")
        .status()?;
    assert!(status.success(), "{status}");

    Ok(())
}

fn test_api_version(api_url: &Url) -> anyhow::Result<()> {
    println!("Testing API deploy is version {API_VERSION} at {api_url}");

    let output = Command::new("cargo")
        .args(["run", "--", "server", "version", "--host", api_url.as_ref()])
        .current_dir("./services/cli")
        .output()?;

    eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    println!("{}", String::from_utf8_lossy(&output.stdout));
    output.status.success().then_some(()).ok_or_else(|| {
        anyhow::anyhow!(
            "Failed to get server version. Exit code: {:?}",
            output.status.code()
        )
    })?;

    let api_version =
        serde_json::from_str::<JsonApiVersion>(std::str::from_utf8(&output.stdout)?)?.version;
    if api_version != API_VERSION {
        return Err(anyhow::anyhow!(
            "API version {api_version} does not match current version {API_VERSION}"
        ));
    }

    Ok(())
}

#[derive(Clone, Copy)]
enum MockSetup {
    BencherCloud,
    SelfHosted { examples: bool, runner: bool },
}

fn test(api_url: &Url, registry_url: &Url, mock_setup: MockSetup) -> anyhow::Result<()> {
    #[cfg(not(feature = "plus"))]
    let _unused = registry_url;
    match mock_setup {
        MockSetup::BencherCloud => {
            let task = TaskSeedTest {
                url: Some(api_url.clone()),
                is_bencher_cloud: true,
                ..Default::default()
            };
            let seed_test = SeedTest::try_from(task)?;
            seed_test.exec()?;

            #[cfg(feature = "plus")]
            {
                // Run OCI conformance tests
                let oci = Oci::try_from(TaskOci {
                    url: Some(registry_url.clone()),
                    ..Default::default()
                })?;
                oci.exec()?;

                // Run runner smoke test (requires Docker + KVM for the runner daemon)
                let runner_test = runner::RunnerTest::try_from(TaskRunner {
                    url: Some(api_url.clone()),
                    with_daemon: true,
                    ..Default::default()
                })?;
                runner_test.exec()?;
            }

            move_project_to_bmf_v1(&seed_test)?;
            send_bmf_v1_report(&seed_test)
        },
        MockSetup::SelfHosted { examples, runner } => {
            let task = TaskSeedTest {
                url: Some(api_url.clone()),
                ..Default::default()
            };
            let seed_test = SeedTest::try_from(task)?;
            seed_test.exec()?;

            #[cfg(feature = "plus")]
            {
                // Run OCI conformance tests
                let oci = Oci::try_from(TaskOci {
                    url: Some(registry_url.clone()),
                    ..Default::default()
                })?;
                oci.exec()?;

                // Run runner smoke test (requires Docker + KVM for the runner daemon)
                if runner {
                    let runner_test = runner::RunnerTest::try_from(TaskRunner {
                        url: Some(api_url.clone()),
                        with_daemon: true,
                        ..Default::default()
                    })?;
                    runner_test.exec()?;
                }
            }
            #[cfg(not(feature = "plus"))]
            let _ = runner;

            if examples {
                let examples = Examples::try_from(TaskExamples {
                    url: Some(api_url.clone()),
                    ..Default::default()
                })?;
                examples.exec()?;
            }

            move_project_to_bmf_v1(&seed_test)?;
            send_bmf_v1_report(&seed_test)
        },
    }
}

// The dev deploy leaves a BMF v1 project to look at in the new console.
fn move_project_to_bmf_v1(seed_test: &SeedTest) -> anyhow::Result<()> {
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "project",
        "update",
        HOST_ARG,
        seed_test.url.as_ref(),
        TOKEN_ARG,
        seed_test.admin_token.as_ref(),
        "--bmf-version",
        "1",
        PROJECT_SLUG,
    ])
    .current_dir(CLI_DIR);
    let assert = cmd.assert().success();
    let project: JsonProject = serde_json::from_slice(&assert.get_output().stdout)?;
    anyhow::ensure!(
        project.bmf_version == BmfVersion::V1,
        "Project {PROJECT_SLUG} is on BMF version {} after moving it to 1",
        project.bmf_version
    );
    Ok(())
}

fn send_bmf_v1_report(seed_test: &SeedTest) -> anyhow::Result<()> {
    let mut cmd = assert_cmd::Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "run",
        HOST_ARG,
        seed_test.url.as_ref(),
        TOKEN_ARG,
        seed_test.token.as_ref(),
        PROJECT_ARG,
        PROJECT_SLUG,
        BRANCH_ARG,
        BRANCH_SLUG,
        TESTBED_ARG,
        TESTBED_SLUG,
        "--adapter",
        "json",
        "--format",
        "json",
        "--quiet",
    ])
    .current_dir(CLI_DIR)
    .write_stdin(BMF_V1_REPORT);
    let assert = cmd.assert().success();
    let report: JsonReport = serde_json::from_slice(&assert.get_output().stdout)?;
    anyhow::ensure!(
        report.bmf_version == Some(BmfVersion::V1),
        "Report on {PROJECT_SLUG} was read at BMF version {:?}, not 1",
        report.bmf_version
    );
    anyhow::ensure!(
        report.parse_outcome == Some(ParseOutcome::Parsed),
        "Report on {PROJECT_SLUG} has parse outcome {:?}, not parsed",
        report.parse_outcome
    );
    anyhow::ensure!(
        report.warnings.is_none(),
        "Report on {PROJECT_SLUG} has warnings {:?}",
        report.warnings
    );
    let metrics = report
        .results
        .iter()
        .flatten()
        .flatten()
        .flat_map(|result| {
            result.measures.iter().flat_map(move |measure| {
                measure.metrics.iter().map(move |metric| {
                    format!(
                        "{} {} {} {} {}",
                        result.benchmark.name,
                        result.variant.parameters,
                        measure.measure.slug,
                        metric.name,
                        metric.value
                    )
                })
            })
        })
        .collect::<BTreeSet<_>>();
    let payload_metrics = BTreeSet::from(BMF_V1_METRICS.map(String::from));
    anyhow::ensure!(
        metrics == payload_metrics,
        "Report on {PROJECT_SLUG} holds {metrics:#?}, not the payload's {payload_metrics:#?}"
    );
    let payload_counts = vec![JsonReportIterationCounts {
        benchmarks: 4,
        measures: 1,
        lines: 12,
    }];
    anyhow::ensure!(
        report.counts.results == payload_counts,
        "Report on {PROJECT_SLUG} counts {:?}, not the payload's {payload_counts:?}",
        report.counts.results
    );
    Ok(())
}

const BMF_V1_REPORT: &str = r#"{
    "bmf_v1::hash": [
        {
            "measures": {
                "latency": { "mean": 41.5, "median": 40.0, "std_dev": 2.25 }
            }
        }
    ],
    "bmf_v1::search": [
        {
            "parameters": { "hit": true },
            "measures": {
                "latency": { "mean": 96.5, "median": 95.0, "std_dev": 4.25 }
            }
        }
    ],
    "bmf_v1::sort": [
        {
            "parameters": { "length": 1024, "order": "random" },
            "measures": {
                "latency": { "mean": 3125.5, "median": 3050.0, "std_dev": 112.75 }
            }
        },
        {
            "parameters": { "length": 1024, "order": "sorted" },
            "measures": {
                "latency": { "mean": 812.25, "median": 800.0, "std_dev": 18.5 }
            }
        }
    ]
}"#;

const BMF_V1_METRICS: [&str; 12] = [
    "bmf_v1::hash {} latency mean 41.5",
    "bmf_v1::hash {} latency median 40",
    "bmf_v1::hash {} latency std_dev 2.25",
    r#"bmf_v1::search {"hit":true} latency mean 96.5"#,
    r#"bmf_v1::search {"hit":true} latency median 95"#,
    r#"bmf_v1::search {"hit":true} latency std_dev 4.25"#,
    r#"bmf_v1::sort {"length":1024,"order":"random"} latency mean 3125.5"#,
    r#"bmf_v1::sort {"length":1024,"order":"random"} latency median 3050"#,
    r#"bmf_v1::sort {"length":1024,"order":"random"} latency std_dev 112.75"#,
    r#"bmf_v1::sort {"length":1024,"order":"sorted"} latency mean 812.25"#,
    r#"bmf_v1::sort {"length":1024,"order":"sorted"} latency median 800"#,
    r#"bmf_v1::sort {"length":1024,"order":"sorted"} latency std_dev 18.5"#,
];

fn kill_child(child: Option<Child>) -> anyhow::Result<()> {
    child
        .expect("Child process is expected for `localhost`")
        .kill()
        .map_err(Into::into)
}
