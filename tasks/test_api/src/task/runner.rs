use std::process::Command;

use assert_cmd::{assert::OutputAssertExt as _, cargo::CommandCargoExt as _};
use bencher_json::{DEV_BENCHER_API_URL, Jwt, LOCALHOST_BENCHER_API_URL, Url};
use pretty_assertions::assert_eq;

use crate::parser::TaskRunner;
use crate::task::test::seed_test::{BENCHER_CMD, CLI_DIR, HOST_ARG, PROJECT_SLUG, TOKEN_ARG};

const DOCKER_IMAGE: &str = "ghcr.io/bencherdev/bencher:latest";
const IMAGE_TAG: &str = "runner-test";
pub(crate) const OCI_USERNAME: &str = "muriel.bagge@nowhere.com";

#[derive(Debug)]
pub struct RunnerTest {
    url: Url,
    token: Jwt,
    username: String,
    admin_token: Option<Jwt>,
    with_daemon: bool,
}

impl TryFrom<TaskRunner> for RunnerTest {
    type Error = anyhow::Error;

    fn try_from(runner: TaskRunner) -> Result<Self, Self::Error> {
        let TaskRunner {
            url,
            token,
            username,
            admin_token,
            with_daemon,
        } = runner;

        let is_dev = url
            .as_ref()
            .is_some_and(|u| u.as_ref() == DEV_BENCHER_API_URL.as_str());

        let token = token.unwrap_or_else(|| {
            if is_dev {
                use crate::task::test::smoke_test::DEV_BENCHER_API_TOKEN;
                DEV_BENCHER_API_TOKEN.clone()
            } else {
                Jwt::test_token()
            }
        });

        if with_daemon {
            anyhow::ensure!(
                admin_token.is_some(),
                "--admin-token is required when using --with-daemon"
            );
        }

        Ok(Self {
            url: url.unwrap_or_else(|| LOCALHOST_BENCHER_API_URL.clone().into()),
            token,
            username: username.unwrap_or_else(|| OCI_USERNAME.to_owned()),
            admin_token,
            with_daemon,
        })
    }
}

impl RunnerTest {
    pub fn exec(&self) -> anyhow::Result<()> {
        if self.with_daemon {
            #[cfg(feature = "plus")]
            return self.exec_with_daemon();
            #[cfg(not(feature = "plus"))]
            anyhow::bail!("--with-daemon requires the `plus` feature");
        }
        run_runner_test(&self.url, &self.token, &self.username)
    }

    #[cfg(feature = "plus")]
    fn exec_with_daemon(&self) -> anyhow::Result<()> {
        use assert_cmd::cargo::CommandCargoExt as _;

        let is_linux = cfg!(target_os = "linux");
        let has_kvm = is_linux && std::path::Path::new("/dev/kvm").exists();

        if !has_kvm {
            println!("Skipping runner test: requires Linux + KVM");
            return Ok(());
        }

        if !docker_available() {
            println!("Skipping runner test: Docker not available");
            return Ok(());
        }

        println!("=== Runner Daemon Test ===");

        let admin_token = self
            .admin_token
            .as_ref()
            .expect("admin_token checked in TryFrom");
        let host = self.url.as_ref();

        // Rotate the runner token to get a fresh one we can use
        #[expect(deprecated)]
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "runner",
            "token",
            HOST_ARG,
            host,
            TOKEN_ARG,
            admin_token.as_ref(),
            "test-runner",
        ])
        .current_dir(CLI_DIR);
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
                HOST_ARG,
                host,
                TOKEN_ARG,
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
            std::time::Duration::from_secs(30),
        );

        // Run the actual runner test
        let result = run_runner_test(&self.url, &self.token, &self.username);

        // Always kill the runner daemon, even if the test failed
        let _kill = runner_child.kill();
        let _wait = runner_child.wait();
        let _join = reader_handle.join();

        result?;
        println!("=== Runner Daemon Test Passed ===");
        Ok(())
    }
}

/// Check whether Docker is available.
pub fn docker_available() -> bool {
    Command::new("docker")
        .args(["version"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Run the runner smoke test: pull a prebuilt image, push it to the API's OCI
/// registry via Docker, then submit a job with `bencher run --image`.
pub fn run_runner_test(url: &Url, token: &Jwt, username: &str) -> anyhow::Result<()> {
    let host = url.as_ref();

    println!("Running runner smoke test against: {host}");

    // Extract the registry host (e.g. "localhost:61016") from the URL.
    let registry = registry_host(host)?;

    // Step 1: Pull the prebuilt Docker image
    println!("Step 1: Pulling Docker image {DOCKER_IMAGE}...");
    docker_pull(DOCKER_IMAGE)?;

    // Step 2: Tag the image for the local registry
    let local_ref = format!("{registry}/{PROJECT_SLUG}:{IMAGE_TAG}");
    println!("Step 2: Tagging image as {local_ref}...");
    docker_tag(DOCKER_IMAGE, &local_ref)?;

    // Step 3: Log in to the local OCI registry
    println!("Step 3: Logging in to {registry}...");
    docker_login(&registry, username, token.as_ref())?;

    // Step 4: Push the image to the local registry
    println!("Step 4: Pushing image to {registry}...");
    docker_push(&local_ref)?;

    // Step 5: Submit a job via `bencher run --image`
    println!("Step 5: Submitting job via bencher run --image...");
    #[expect(deprecated)]
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "run",
        HOST_ARG,
        host,
        TOKEN_ARG,
        token.as_ref(),
        "--project",
        PROJECT_SLUG,
        "--branch",
        "master",
        "--testbed",
        "base",
        "--image",
        &format!("{PROJECT_SLUG}:{IMAGE_TAG}"),
        "--spec",
        "test-spec",
        "--format",
        "json",
        "--quiet",
        "--job-timeout",
        "120",
        "--poll-interval",
        "2",
        "--exec",
        "mock",
    ])
    .current_dir(CLI_DIR);
    let assert = cmd.assert().success();

    // Step 6: Verify the results
    println!("Step 6: Verifying results...");
    let json: bencher_json::JsonReport = serde_json::from_slice(&assert.get_output().stdout)?;
    assert_eq!(json.project.slug.to_string(), PROJECT_SLUG);
    #[cfg(feature = "plus")]
    assert!(json.job.is_some(), "Expected job UUID in report: {json:?}");

    println!("Runner smoke test passed!");
    Ok(())
}

/// Extract the host:port portion of a URL string for use as a Docker registry address.
///
/// Strips the scheme (e.g. `http://`) and any trailing path, returning just `host:port`.
fn registry_host(url: &str) -> anyhow::Result<String> {
    let without_scheme = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);
    // Take everything up to the first `/` (path separator)
    let authority = without_scheme.split('/').next().unwrap_or(without_scheme);
    anyhow::ensure!(!authority.is_empty(), "URL {url} has no host");
    Ok(authority.to_owned())
}

fn docker_pull(image: &str) -> anyhow::Result<()> {
    let status = Command::new("docker").args(["pull", image]).status()?;
    anyhow::ensure!(status.success(), "docker pull {image} failed: {status}");
    Ok(())
}

fn docker_tag(src: &str, dst: &str) -> anyhow::Result<()> {
    let status = Command::new("docker").args(["tag", src, dst]).status()?;
    anyhow::ensure!(status.success(), "docker tag {src} {dst} failed: {status}");
    Ok(())
}

fn docker_login(registry: &str, username: &str, password: &str) -> anyhow::Result<()> {
    let output = Command::new("docker")
        .args(["login", registry, "-u", username, "--password-stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write as _;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(password.as_bytes())?;
            }
            child.wait_with_output()
        })?;
    anyhow::ensure!(
        output.status.success(),
        "docker login to {registry} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn docker_push(image: &str) -> anyhow::Result<()> {
    let status = Command::new("docker").args(["push", image]).status()?;
    anyhow::ensure!(status.success(), "docker push {image} failed: {status}");
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
    child: &mut std::process::Child,
    sentinel: &str,
    label: &str,
    timeout: std::time::Duration,
) -> std::thread::JoinHandle<()> {
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

    let handle = std::thread::spawn(move || {
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
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    handle
}
