use std::process::Command;

use assert_cmd::{assert::OutputAssertExt as _, cargo::CommandCargoExt as _};
use bencher_json::{Jwt, LOCALHOST_BENCHER_API_URL, Url};
use pretty_assertions::assert_eq;

use crate::parser::TaskRunner;
use crate::task::test::seed_test::{BENCHER_CMD, CLI_DIR, HOST_ARG, PROJECT_SLUG, TOKEN_ARG};

const DOCKER_IMAGE: &str = "ghcr.io/bencherdev/bencher:latest";
const IMAGE_TAG: &str = "runner-test";
const OCI_USERNAME: &str = "muriel.bagge@nowhere.com";

#[derive(Debug)]
pub struct RunnerTest {
    url: Url,
    token: Jwt,
}

impl TryFrom<TaskRunner> for RunnerTest {
    type Error = anyhow::Error;

    fn try_from(runner: TaskRunner) -> Result<Self, Self::Error> {
        let TaskRunner { url, token } = runner;
        Ok(Self {
            url: url.unwrap_or_else(|| LOCALHOST_BENCHER_API_URL.clone().into()),
            token: token.unwrap_or_else(Jwt::test_token),
        })
    }
}

impl RunnerTest {
    pub fn exec(&self) -> anyhow::Result<()> {
        run_runner_test(&self.url, &self.token)
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
pub fn run_runner_test(url: &Url, token: &Jwt) -> anyhow::Result<()> {
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
    docker_login(&registry, OCI_USERNAME, token.as_ref())?;

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
