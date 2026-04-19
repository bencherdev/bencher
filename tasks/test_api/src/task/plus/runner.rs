use std::process::Command;

use assert_cmd::cargo::CommandCargoExt as _;
use bencher_json::{Jwt, Url};
use pretty_assertions::assert_eq;

use crate::parser::TaskRunner;
use crate::task::test::seed_test::{
    BENCHER_CMD, CLI_DIR, HOST_ARG, PROJECT_SLUG, TOKEN_ARG, USER_EMAIL,
};
use crate::task::{is_dev, unwrap_admin_token, unwrap_url, unwrap_user_token};

const DOCKER_IMAGE: &str = "ghcr.io/bencherdev/bencher:latest";
const IMAGE_TAG: &str = "runner-test";

#[derive(Debug)]
pub struct RunnerTest {
    url: Url,
    admin_token: Jwt,
    username: String,
    token: Jwt,
    with_daemon: bool,
}

impl TryFrom<TaskRunner> for RunnerTest {
    type Error = anyhow::Error;

    fn try_from(runner: TaskRunner) -> Result<Self, Self::Error> {
        let TaskRunner {
            url,
            admin_token,
            username,
            token,
            with_daemon,
        } = runner;

        let is_dev = is_dev(url.as_ref());
        let url = unwrap_url(url);
        let admin_token = unwrap_admin_token(admin_token, is_dev);
        // Run tests as a normal user
        let username = username.unwrap_or_else(|| USER_EMAIL.to_owned());
        let token = unwrap_user_token(token, is_dev);

        Ok(Self {
            url,
            admin_token,
            username,
            token,
            with_daemon,
        })
    }
}

impl RunnerTest {
    pub fn exec(&self) -> anyhow::Result<()> {
        if self.with_daemon {
            self.exec_with_daemon()
        } else {
            if docker_available() {
                // Only test the unauthenticated push (no `bencher run --image`)
                // since there is no runner daemon to execute the remote job.
                run_unclaimed_push_test(&self.url)?;
            }
            let spec = if cfg!(target_os = "linux") {
                "test-spec"
            } else {
                "no-sandbox-spec"
            };
            run_runner_test(&self.url, &self.username, &self.token, spec)
        }
    }

    #[expect(clippy::too_many_lines)]
    fn exec_with_daemon(&self) -> anyhow::Result<()> {
        if !docker_available() {
            println!("Skipping runner test: Docker not available");
            return Ok(());
        }

        println!("=== Runner Daemon Test ===");

        let host = self.url.as_ref();

        // Rotate the runner key to get a fresh one we can use
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "runner",
            "key",
            HOST_ARG,
            host,
            TOKEN_ARG,
            self.admin_token.as_ref(),
            "test-runner",
        ])
        .current_dir(CLI_DIR);
        let output = cmd.output()?;
        anyhow::ensure!(
            output.status.success(),
            "Failed to rotate runner key: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let runner_key: bencher_json::JsonRunnerKey = serde_json::from_slice(&output.stdout)?;

        // Rotate the no-sandbox runner key
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "runner",
            "key",
            HOST_ARG,
            host,
            TOKEN_ARG,
            self.admin_token.as_ref(),
            "test-runner-no-sandbox",
        ])
        .current_dir(CLI_DIR);
        let output = cmd.output()?;
        anyhow::ensure!(
            output.status.success(),
            "Failed to rotate no-sandbox runner key: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let no_sandbox_runner_key: bencher_json::JsonRunnerKey =
            serde_json::from_slice(&output.stdout)?;

        // On Linux with KVM, build bencher-init for the musl target so it can
        // be bundled into the runner binary. On other platforms (e.g. macOS),
        // the runner runs in debug mode without KVM and doesn't need bencher-init.
        let is_linux = cfg!(target_os = "linux");
        let has_kvm = is_linux && camino::Utf8Path::new("/dev/kvm").exists();

        if has_kvm {
            let workspace_root = camino::Utf8PathBuf::try_from(std::env::current_dir()?)
                .expect("workspace root should be valid UTF-8");
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
            anyhow::ensure!(init_path.exists(), "bencher-init not found at {init_path}");

            println!("Building runner (BENCHER_INIT_PATH={init_path})...");
            let build_status = Command::new("cargo")
                .args(["build", "--bin", "runner"])
                .env("BENCHER_INIT_PATH", &init_path)
                .status()?;
            anyhow::ensure!(build_status.success(), "Failed to build runner binary");
        } else {
            println!("Building runner (debug mode, no KVM)...");
            let build_status = Command::new("cargo")
                .args(["build", "--bin", "runner"])
                .status()?;
            anyhow::ensure!(build_status.success(), "Failed to build runner binary");

            println!("Building bencher CLI...");
            let build_status = Command::new("cargo")
                .args(["build", "--bin", "bencher"])
                .status()?;
            anyhow::ensure!(build_status.success(), "Failed to build bencher CLI");
        }

        // Start the Firecracker runner daemon only when KVM is available
        let runner_child_and_handle = if has_kvm {
            println!("Starting runner daemon...");
            let mut runner_child = Command::cargo_bin("runner")?;
            let mut runner_child = runner_child
                .args([
                    "up",
                    HOST_ARG,
                    host,
                    "--key",
                    runner_key.key.as_ref(),
                    "--runner",
                    "test-runner",
                ])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::inherit())
                .spawn()?;

            let reader_handle = wait_for_stdout_ready(
                &mut runner_child,
                "Polling for jobs",
                "runner",
                std::time::Duration::from_secs(30),
            );
            Some((runner_child, reader_handle))
        } else {
            println!("Skipping Firecracker runner daemon (no KVM)");
            None
        };

        // Start the no-sandbox runner daemon
        println!("Starting no-sandbox runner daemon...");
        let mut no_sandbox_child = Command::cargo_bin("runner")?;
        let mut no_sandbox_child = no_sandbox_child
            .args([
                "up",
                HOST_ARG,
                host,
                "--key",
                no_sandbox_runner_key.key.as_ref(),
                "--runner",
                "test-runner-no-sandbox",
                "--danger-allow-no-sandbox",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;

        let no_sandbox_reader_handle = wait_for_stdout_ready(
            &mut no_sandbox_child,
            "Polling for jobs",
            "no-sandbox-runner",
            std::time::Duration::from_secs(30),
        );

        // Run unclaimed project test (no docker login, no token)
        let unclaimed_result = run_unclaimed_test(&self.url);

        // Run the actual runner test (use no-sandbox spec when KVM is unavailable)
        let spec = if has_kvm {
            "test-spec"
        } else {
            "no-sandbox-spec"
        };
        let result = if unclaimed_result.is_ok() {
            run_runner_test(&self.url, &self.username, &self.token, spec)
        } else {
            Ok(())
        };

        // Run the no-sandbox runner test
        let no_sandbox_result = if result.is_ok() {
            run_no_sandbox_runner_test(&self.url, &self.token)
        } else {
            Ok(())
        };

        // Run the detach runner test
        let detach_result = if no_sandbox_result.is_ok() {
            run_detach_runner_test(&self.url, &self.token)
        } else {
            Ok(())
        };

        // Always kill runner daemons, even if the test failed
        if let Some((mut runner_child, reader_handle)) = runner_child_and_handle {
            let _kill = runner_child.kill();
            let _wait = runner_child.wait();
            let _join = reader_handle.join();
        }
        let _kill = no_sandbox_child.kill();
        let _wait = no_sandbox_child.wait();
        let _join = no_sandbox_reader_handle.join();

        unclaimed_result?;
        result?;
        no_sandbox_result?;
        detach_result?;
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
pub fn run_runner_test(url: &Url, username: &str, token: &Jwt, spec: &str) -> anyhow::Result<()> {
    let host = url.as_ref();

    println!("Running runner smoke test against: {host}");

    // On macOS, Docker Desktop runs the daemon in a VM where localhost is the
    // VM's loopback, not the host. We need host.docker.internal for Docker
    // commands, and the API server's registry_url must also use it so the auth
    // realm URL is reachable from Docker's daemon.
    // The runner daemon on the host also needs to resolve host.docker.internal,
    // so we ensure it's in /etc/hosts.
    let registry = if cfg!(target_os = "macos") {
        let port = registry_host(host)?
            .rsplit_once(':')
            .and_then(|(_, p)| p.parse::<u16>().ok())
            .unwrap_or(bencher_json::BENCHER_API_PORT);
        let docker_registry = format!("host.docker.internal:{port}");
        println!("macOS detected, using Docker registry host: {docker_registry}");
        ensure_hosts_entry()?;
        ensure_insecure_registry(&docker_registry)?;
        docker_registry
    } else {
        registry_for_api(host)?
    };

    let local_ref = format!("{registry}/{PROJECT_SLUG}:{IMAGE_TAG}");
    if cfg!(target_os = "macos") {
        // Build a local OCI image containing the macOS-native bencher binary
        println!("Step 1: Building local Docker image from macOS bencher binary...");
        docker_build_local_image(&local_ref)?;
    } else {
        // Pull the prebuilt Linux Docker image
        println!("Step 1: Pulling Docker image {DOCKER_IMAGE}...");
        docker_pull(DOCKER_IMAGE)?;
        println!("Step 2: Tagging image as {local_ref}...");
        docker_tag(DOCKER_IMAGE, &local_ref)?;
    }

    // Step 3: Log in to the local OCI registry
    println!("Step 3: Logging in to {registry}...");
    docker_login(&registry, username, token.as_ref())?;

    // Step 4: Push the image to the local registry
    println!("Step 4: Pushing image to {registry}...");
    docker_push(&local_ref)?;

    // Step 5: Submit a job via `bencher run --image`
    println!("Step 5: Submitting job via bencher run --image...");
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    let image_ref = format!("{PROJECT_SLUG}:{IMAGE_TAG}");
    let args = [
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
        &image_ref,
        "--spec",
        spec,
        "--format",
        "json",
        "--quiet",
        "--job-timeout",
        "120",
        "--job-poll-interval",
        "1",
        "--exec",
        "mock",
    ];
    cmd.args(args).current_dir(CLI_DIR);
    let output = cmd.output()?;
    anyhow::ensure!(
        output.status.success(),
        "bencher run failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Step 6: Verify the results
    println!("Step 6: Verifying results...");
    let json: bencher_json::JsonReport = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json.project.slug.to_string(), PROJECT_SLUG);
    #[cfg(feature = "plus")]
    assert!(json.job.is_some(), "Expected job UUID in report: {json:?}");

    println!("Runner smoke test passed!");
    Ok(())
}

/// Run the no-sandbox runner smoke test variant.
///
/// Similar to `run_runner_test` but submits to the `no-sandbox-spec` spec
/// which does not use Firecracker sandboxing.
fn run_no_sandbox_runner_test(url: &Url, token: &Jwt) -> anyhow::Result<()> {
    let host = url.as_ref();

    println!("Running no-sandbox runner smoke test against: {host}");

    // The image should already be pushed from the first test
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    let image_ref = format!("{PROJECT_SLUG}:{IMAGE_TAG}");
    let args = [
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
        &image_ref,
        "--spec",
        "no-sandbox-spec",
        "--format",
        "json",
        "--quiet",
        "--job-timeout",
        "120",
        "--job-poll-interval",
        "1",
        "--exec",
        "mock",
    ];
    cmd.args(args).current_dir(CLI_DIR);
    let output = cmd.output()?;
    anyhow::ensure!(
        output.status.success(),
        "bencher run (no-sandbox) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: bencher_json::JsonReport = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json.project.slug.to_string(), PROJECT_SLUG);
    #[cfg(feature = "plus")]
    assert!(
        json.job.is_some(),
        "Expected job UUID in no-sandbox report: {json:?}"
    );

    println!("No-sandbox runner smoke test passed!");
    Ok(())
}

/// Run a detach runner smoke test: submit a job with `--detach` and verify
/// it returns immediately with a `JsonReport` containing a job UUID,
/// then poll `bencher job view` until the job reaches a terminal state.
fn run_detach_runner_test(url: &Url, token: &Jwt) -> anyhow::Result<()> {
    let host = url.as_ref();

    println!("Running detach runner smoke test against: {host}");

    // Submit the job with --detach (returns immediately)
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    let image_ref = format!("{PROJECT_SLUG}:{IMAGE_TAG}");
    let args = [
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
        &image_ref,
        "--spec",
        "no-sandbox-spec",
        "--format",
        "json",
        "--quiet",
        "--job-timeout",
        "120",
        "--detach",
        "--exec",
        "mock",
    ];
    cmd.args(args).current_dir(CLI_DIR);
    let output = cmd.output()?;
    anyhow::ensure!(
        output.status.success(),
        "bencher run --detach failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: bencher_json::JsonReport = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json.project.slug.to_string(), PROJECT_SLUG);
    let job_uuid = json
        .job
        .ok_or_else(|| anyhow::anyhow!("Expected job UUID in detach report: {json:?}"))?;

    // Poll `bencher job view` until the job reaches a terminal state
    let job_uuid_str = job_uuid.to_string();
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(240);

    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));

        if start.elapsed() > timeout {
            anyhow::bail!("Timed out waiting for detached job {job_uuid} to complete");
        }

        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "job",
            "view",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token.as_ref(),
            PROJECT_SLUG,
            &job_uuid_str,
        ])
        .current_dir(CLI_DIR);
        let output = cmd.output()?;
        anyhow::ensure!(
            output.status.success(),
            "bencher job view failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let job: bencher_json::JsonJob = serde_json::from_slice(&output.stdout)?;
        match job.status {
            bencher_json::JobStatus::Processed => {
                println!("Detached job {job_uuid} processed successfully");
                break;
            },
            bencher_json::JobStatus::Failed => {
                anyhow::bail!("Detached job {job_uuid} failed: {job:?}");
            },
            bencher_json::JobStatus::Canceled => {
                anyhow::bail!("Detached job {job_uuid} was canceled: {job:?}");
            },
            bencher_json::JobStatus::Pending
            | bencher_json::JobStatus::Claimed
            | bencher_json::JobStatus::Running
            | bencher_json::JobStatus::Completed => {},
        }
    }

    println!("Detach runner smoke test passed!");
    Ok(())
}

/// Ensure that `host.docker.internal` resolves on the host by adding it to
/// `/etc/hosts` if not already present. Requires `sudo`.
fn ensure_hosts_entry() -> anyhow::Result<()> {
    let hosts = std::fs::read_to_string("/etc/hosts")?;
    if hosts.contains("host.docker.internal") {
        println!("host.docker.internal already in /etc/hosts.");
        return Ok(());
    }

    let entry = "127.0.0.1 host.docker.internal";
    anyhow::bail!(
        "host.docker.internal is not in /etc/hosts.\n\
         Run this once to fix it:\n\n\
         echo '{entry}' | sudo tee -a /etc/hosts\n"
    );
}

/// Ensure that the given registry is listed as an insecure registry in
/// Docker Desktop's `~/.docker/daemon.json`.
///
/// If the registry is already configured, this is a no-op.
/// Otherwise, it adds the entry to `daemon.json` and restarts Docker Desktop.
fn ensure_insecure_registry(registry: &str) -> anyhow::Result<()> {
    let info_output = Command::new("docker")
        .args(["info", "--format", "{{json .RegistryConfig.IndexConfigs}}"])
        .output()?;
    if info_output.status.success() {
        let info_str = String::from_utf8_lossy(&info_output.stdout);
        if let Ok(configs) = serde_json::from_str::<serde_json::Value>(info_str.trim())
            && configs.get(registry).is_some()
        {
            println!("Insecure registry '{registry}' is already configured.");
            return Ok(());
        }
    }

    println!("Configuring '{registry}' as an insecure Docker registry...");

    let home = std::env::var("HOME")?;
    let daemon_json_path = std::path::PathBuf::from(home).join(".docker/daemon.json");

    let mut config: serde_json::Value = if daemon_json_path.exists() {
        let contents = std::fs::read_to_string(&daemon_json_path)?;
        serde_json::from_str(&contents)?
    } else {
        serde_json::json!({})
    };

    let registries = config
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("daemon.json is not a JSON object"))?
        .entry("insecure-registries")
        .or_insert_with(|| serde_json::json!([]));
    let arr = registries
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("insecure-registries is not an array"))?;
    let registry_value = serde_json::Value::String(registry.to_owned());
    if !arr.contains(&registry_value) {
        arr.push(registry_value);
    }

    let pretty = serde_json::to_string_pretty(&config)?;
    std::fs::write(&daemon_json_path, &pretty)?;
    println!("Updated {}", daemon_json_path.display());

    // Restart Docker Desktop to pick up the new config.
    println!("Restarting Docker Desktop...");
    drop(Command::new("killall").arg("Docker").status());
    std::thread::sleep(std::time::Duration::from_secs(5));
    drop(Command::new("open").args(["-a", "Docker"]).status());

    // Wait for Docker to become ready.
    println!("Waiting for Docker to be ready...");
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(60);
    loop {
        if docker_available() {
            println!("Docker is ready.");
            return Ok(());
        }
        anyhow::ensure!(
            start.elapsed() < timeout,
            "Timed out waiting for Docker Desktop to restart"
        );
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

/// Map an API URL to its corresponding registry host for Docker operations.
///
/// For known Bencher environments, returns the dedicated registry hostname.
/// For other URLs (e.g. localhost), falls back to extracting the host from the URL.
fn registry_for_api(api_url: &str) -> anyhow::Result<String> {
    if api_url.starts_with(bencher_json::DEV_BENCHER_API_URL_STR) {
        registry_host(bencher_json::DEV_BENCHER_REGISTRY_URL_STR)
    } else if api_url.starts_with(bencher_json::TEST_BENCHER_API_URL_STR) {
        registry_host(bencher_json::TEST_BENCHER_REGISTRY_URL_STR)
    } else if api_url.starts_with(bencher_json::PROD_BENCHER_API_URL_STR) {
        registry_host(bencher_json::PROD_BENCHER_REGISTRY_URL_STR)
    } else {
        registry_host(api_url)
    }
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

/// Prepare an OCI image for unclaimed project testing.
///
/// Resolves the registry for the given API URL, pulls/builds the Docker image,
/// and tags it for the target registry under the given project slug.
/// Returns `(host, local_ref)` for use by push-only or push+run tests.
fn prepare_unclaimed_image(url: &Url, slug: &str) -> anyhow::Result<(String, String)> {
    let host = url.as_ref().to_owned();

    let registry = if cfg!(target_os = "macos") {
        let port = registry_host(&host)?
            .rsplit_once(':')
            .and_then(|(_, p)| p.parse::<u16>().ok())
            .unwrap_or(bencher_json::BENCHER_API_PORT);
        let docker_registry = format!("host.docker.internal:{port}");
        ensure_hosts_entry()?;
        ensure_insecure_registry(&docker_registry)?;
        docker_registry
    } else {
        registry_for_api(&host)?
    };

    let local_ref = format!("{registry}/{slug}:latest");

    if cfg!(target_os = "macos") {
        println!("Building local Docker image from macOS bencher binary...");
        docker_build_local_image(&local_ref)?;
    } else {
        println!("Pulling Docker image {DOCKER_IMAGE}...");
        docker_pull(DOCKER_IMAGE)?;
        println!("Tagging as {local_ref}...");
        docker_tag(DOCKER_IMAGE, &local_ref)?;
    }

    Ok((host, local_ref))
}

/// Push-only smoke test for unclaimed projects.
///
/// Builds/pulls an image and pushes it without `docker login`.
/// Does NOT run `bencher run --image` — use `run_unclaimed_test` for the full
/// push + run test (requires a running runner daemon).
fn run_unclaimed_push_test(url: &Url) -> anyhow::Result<()> {
    println!("=== Unclaimed Push Smoke Test ===");

    let (_host, local_ref) = prepare_unclaimed_image(url, "unclaimed-push-test")?;

    // Intentionally NO docker login
    println!("Pushing without auth...");
    docker_push(&local_ref)?;

    println!("=== Unclaimed Push Smoke Test Passed ===");
    Ok(())
}

/// Run the unclaimed project smoke test.
///
/// Builds/pulls an image, pushes it without `docker login`, then runs
/// `bencher run --image` without a token. Repeats the push + run sequence
/// twice to verify the project stays unclaimed.
fn run_unclaimed_test(url: &Url) -> anyhow::Result<()> {
    println!("=== Unclaimed Project Smoke Test ===");

    let slug = "unclaimed-smoke-test";
    let (host, local_ref) = prepare_unclaimed_image(url, slug)?;

    // Intentionally NO docker login

    // Run the full push + run sequence twice to verify the project stays unclaimed
    run_unclaimed_push_and_run(&host, &local_ref, slug, 1)?;
    run_unclaimed_push_and_run(&host, &local_ref, slug, 2)?;

    println!("=== Unclaimed Project Smoke Test Passed ===");
    Ok(())
}

/// Push an image without auth and run `bencher run --image` without a token.
fn run_unclaimed_push_and_run(
    host: &str,
    local_ref: &str,
    slug: &str,
    run: u32,
) -> anyhow::Result<()> {
    println!("--- Run {run}/2 ---");

    println!("Pushing without auth...");
    docker_push(local_ref)?;

    println!("Running bencher run --image without token...");
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    let image_ref = format!("{slug}:latest");
    let args = [
        "run",
        HOST_ARG,
        host,
        // No TOKEN_ARG — fully unauthenticated
        "--project",
        slug,
        "--branch",
        "master",
        "--testbed",
        "base",
        "--image",
        &image_ref,
        "--spec",
        "no-sandbox-spec",
        "--format",
        "json",
        "--quiet",
        "--job-timeout",
        "120",
        "--job-poll-interval",
        "1",
        "--exec",
        "mock",
    ];
    cmd.args(args).current_dir(CLI_DIR);
    let output = cmd.output()?;
    anyhow::ensure!(
        output.status.success(),
        "bencher run (unclaimed, run {run}) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: bencher_json::JsonReport = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json.project.slug.to_string(), slug);
    println!("Run {run}/2 passed!");
    Ok(())
}

/// Build a Docker image containing the locally-compiled bencher binary.
///
/// Creates a minimal `FROM scratch` image with the binary at `/usr/bin/bencher`,
/// matching the production image layout. This allows macOS tests to use a native
/// binary inside the OCI image instead of overriding the entrypoint.
fn docker_build_local_image(tag: &str) -> anyhow::Result<()> {
    let bencher_bin = assert_cmd::cargo::cargo_bin(BENCHER_CMD);
    let build_context = bencher_bin.parent().expect("binary should have parent dir");

    let dockerfile =
        "FROM scratch\nCOPY bencher /usr/bin/bencher\nENTRYPOINT [\"/usr/bin/bencher\"]\n";
    let dockerfile_path = std::env::temp_dir().join("Dockerfile.bencher-runner-test");
    std::fs::write(&dockerfile_path, dockerfile)?;

    let status = Command::new("docker")
        .args(["build", "-t", tag, "-f"])
        .arg(&dockerfile_path)
        .arg(build_context)
        .status()?;

    drop(std::fs::remove_file(&dockerfile_path));
    anyhow::ensure!(status.success(), "docker build failed");
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
    let thread_sentinel = sentinel.to_owned();
    let thread_label = label.to_owned();

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
