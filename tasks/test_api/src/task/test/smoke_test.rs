use std::sync::LazyLock;
use std::{
    net::TcpStream,
    process::{Child, Command},
    thread,
    time::Duration,
};

use bencher_json::{
    BENCHER_API_PORT, DEV_BENCHER_API_URL, JsonApiVersion, Jwt, LOCALHOST_BENCHER_API_URL,
    PROD_BENCHER_API_URL, TEST_BENCHER_API_URL, Url,
};

use crate::{
    API_VERSION,
    parser::{TaskExamples, TaskOci, TaskSeedTest, TaskSmokeTest, TaskTestEnvironment},
    task::{
        oci::Oci,
        runner,
        test::{examples::Examples, seed_test::SeedTest},
    },
};

const DEV_ADMIN_BENCHER_API_TOKEN_STR: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo2MDYwMDI5NDE0LCJpYXQiOjE3NjUwNjIxMTksImlzcyI6Imh0dHBzOi8vZGV2ZWwtLWJlbmNoZXIubmV0bGlmeS5hcHAvIiwic3ViIjoiZXVzdGFjZS5iYWdnZUBub3doZXJlLmNvbSIsIm9yZyI6bnVsbCwic3RhdGUiOm51bGx9.jY6749lVWe3pJ53LBXoNSl19b59xifOLdwMwQUNMZ5g";
const DEV_BENCHER_API_TOKEN_STR: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo1OTkzNjQyMTU2LCJpYXQiOjE2OTg2NzQ4NjEsImlzcyI6Imh0dHBzOi8vZGV2ZWwtLWJlbmNoZXIubmV0bGlmeS5hcHAvIiwic3ViIjoibXVyaWVsLmJhZ2dlQG5vd2hlcmUuY29tIiwib3JnIjpudWxsfQ.9z7jmM53TcVzc1inDxTeX9_OR0PQPpZAsKsCE7lWHfo";

pub static DEV_ADMIN_BENCHER_API_TOKEN: LazyLock<Jwt> = LazyLock::new(|| {
    DEV_ADMIN_BENCHER_API_TOKEN_STR
        .parse()
        .expect("Invalid test JWT")
});

pub static DEV_BENCHER_API_TOKEN: LazyLock<Jwt> =
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
        test_api_version(&api_url)?;

        match self.environment {
            Environment::Ci => {
                test(&api_url, MockSetup::SelfHosted { examples: false })?;
                kill_child(child)?;
            },
            Environment::Localhost => {
                test(&api_url, MockSetup::SelfHosted { examples: true })?;
                kill_child(child)?;
            },
            Environment::Docker => bencher_down()?,
            Environment::Dev => test(
                &api_url,
                MockSetup::BencherCloud {
                    admin_token: DEV_ADMIN_BENCHER_API_TOKEN.clone(),
                    token: DEV_BENCHER_API_TOKEN.clone(),
                },
            )?,
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

enum MockSetup {
    BencherCloud { admin_token: Jwt, token: Jwt },
    SelfHosted { examples: bool },
}

fn test(api_url: &Url, mock_setup: MockSetup) -> anyhow::Result<()> {
    match mock_setup {
        MockSetup::BencherCloud { admin_token, token } => {
            let task = TaskSeedTest {
                url: Some(api_url.clone()),
                admin_token: Some(admin_token.clone()),
                token: Some(token),
                is_bencher_cloud: true,
                no_git: false,
            };
            SeedTest::try_from(task)?.exec()?;

            Ok(())
        },
        MockSetup::SelfHosted { examples } => {
            let task = TaskSeedTest {
                url: Some(api_url.clone()),
                admin_token: None,
                token: None,
                is_bencher_cloud: false,
                no_git: false,
            };
            SeedTest::try_from(task)?.exec()?;

            // Run OCI conformance tests
            let oci = Oci::try_from(TaskOci::for_test(api_url.as_ref(), true))?;
            oci.exec()?;

            // Run runner smoke test (requires Docker + KVM for the runner daemon)
            #[cfg(feature = "plus")]
            run_runner_smoke_test(api_url)?;

            if examples {
                let examples = Examples::try_from(TaskExamples {
                    url: Some(api_url.clone()),
                    token: None,
                    example: None,
                })?;
                examples.exec()?;
            }

            Ok(())
        },
    }
}

fn kill_child(child: Option<Child>) -> anyhow::Result<()> {
    child
        .expect("Child process is expected for `localhost`")
        .kill()
        .map_err(Into::into)
}

/// Run the runner smoke test: rotate a runner token, start the runner daemon,
/// push a Docker image to the API's OCI registry, and submit a job.
#[cfg(feature = "plus")]
fn run_runner_smoke_test(api_url: &Url) -> anyhow::Result<()> {
    use assert_cmd::cargo::CommandCargoExt as _;

    let is_linux = cfg!(target_os = "linux");
    let has_kvm = is_linux && std::path::Path::new("/dev/kvm").exists();

    if !has_kvm {
        println!("Skipping runner smoke test: requires Linux + KVM");
        return Ok(());
    }

    if !runner::docker_available() {
        println!("Skipping runner smoke test: Docker not available");
        return Ok(());
    }

    println!("=== Runner Smoke Test ===");

    let admin_token = Jwt::test_admin_token();
    let token = Jwt::test_token();
    let host = api_url.as_ref();

    // Rotate the runner token to get a fresh one we can use
    #[expect(deprecated)]
    let mut cmd = Command::cargo_bin(super::seed_test::BENCHER_CMD)?;
    cmd.args([
        "runner",
        "token",
        super::seed_test::HOST_ARG,
        host,
        super::seed_test::TOKEN_ARG,
        admin_token.as_ref(),
        "test-runner",
    ])
    .current_dir(super::seed_test::CLI_DIR);
    let output = cmd.output()?;
    anyhow::ensure!(
        output.status.success(),
        "Failed to rotate runner token: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let runner_token: bencher_json::JsonRunnerToken = serde_json::from_slice(&output.stdout)?;

    // Build bencher-init for the musl target so it can be bundled into the runner binary.
    let workspace_root = std::env::current_dir()?;
    let target_triple = musl_target_triple()?;

    println!("Building bencher-init ({target_triple})...");
    let build_status = Command::new("cargo")
        .args(["build", "--target", target_triple, "-p", "bencher_init"])
        .status()?;
    anyhow::ensure!(build_status.success(), "Failed to build bencher-init");

    let init_path = workspace_root
        .join("target")
        .join(target_triple)
        .join("debug")
        .join("bencher-init");
    anyhow::ensure!(
        init_path.exists(),
        "bencher-init not found at {}",
        init_path.display()
    );

    // Build the runner binary with BENCHER_INIT_PATH so the init binary gets bundled.
    println!(
        "Building runner (BENCHER_INIT_PATH={})...",
        init_path.display()
    );
    let build_status = Command::new("cargo")
        .args(["build", "--bin", "runner"])
        .env("BENCHER_INIT_PATH", &init_path)
        .status()?;
    anyhow::ensure!(build_status.success(), "Failed to build runner binary");

    // Start the runner daemon as a background process
    println!("Starting runner daemon...");
    #[expect(deprecated)]
    let mut runner_child = Command::cargo_bin("runner")?;
    let mut runner_child = runner_child
        .args([
            "up",
            super::seed_test::HOST_ARG,
            host,
            super::seed_test::TOKEN_ARG,
            runner_token.token.as_ref(),
            "--runner",
            "test-runner",
        ])
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    // Wait for the runner to be ready instead of sleeping a fixed duration
    let reader_handle = wait_for_stdout_ready(
        &mut runner_child,
        "Polling for jobs",
        "runner",
        Duration::from_secs(30),
    );

    // Run the actual runner test
    let result = runner::run_runner_test(api_url, &token);

    // Always kill the runner daemon, even if the test failed
    let _kill = runner_child.kill();
    let _wait = runner_child.wait();
    let _join = reader_handle.join();

    result?;
    println!("=== Runner Smoke Test Passed ===");
    Ok(())
}

/// Map `std::env::consts::ARCH` to Rust target triples for musl builds.
#[cfg(feature = "plus")]
fn musl_target_triple() -> anyhow::Result<&'static str> {
    use std::env::consts::ARCH;
    match ARCH {
        "x86_64" => Ok("x86_64-unknown-linux-musl"),
        "aarch64" => Ok("aarch64-unknown-linux-musl"),
        arch => anyhow::bail!("Unsupported architecture: {arch}"),
    }
}

/// Waits for a child process to print a line containing `sentinel` to stdout.
///
/// Spawns a reader thread that forwards all stdout lines to the test console
/// (prefixed with `[{label}]`) and sets a flag when the sentinel is found.
/// Returns the reader thread handle for cleanup after the test.
///
/// Panics if the sentinel is not found within `timeout`.
#[cfg(feature = "plus")]
fn wait_for_stdout_ready(
    child: &mut Child,
    sentinel: &str,
    label: &str,
    timeout: Duration,
) -> thread::JoinHandle<()> {
    use std::io::BufRead as _;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;

    let stdout = child
        .stdout
        .take()
        .expect("stdout should be piped for readiness detection");
    let ready = Arc::new(AtomicBool::new(false));
    let ready_clone = Arc::clone(&ready);
    let sentinel = sentinel.to_owned();
    let label = label.to_owned();
    let thread_sentinel = sentinel.clone();
    let thread_label = label.clone();

    let handle = thread::spawn(move || {
        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines() {
            let Ok(line) = line else { break };
            println!("[{thread_label}] {line}");
            if line.contains(&thread_sentinel) {
                ready_clone.store(true, Ordering::SeqCst);
            }
        }
    });

    let start = Instant::now();
    while !ready.load(Ordering::SeqCst) {
        assert!(
            start.elapsed() < timeout,
            "Timed out waiting for '{sentinel}' from [{label}]"
        );
        thread::sleep(Duration::from_millis(100));
    }

    handle
}
