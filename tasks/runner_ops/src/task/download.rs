use std::process::Command;

use anyhow::Context as _;
use bencher_json::{Sha256, UpdateChannel};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;
use sha2::Digest as _;
use tempfile::TempDir;

const REPO: &str = "bencherdev/bencher";
const CANARY: &str = "canary";
const DEVEL: &str = "devel";

/// Download the runner binary and check it against the checksum published beside it.
///
/// Without `run_id` it is the release asset of the update channel: the `canary` release,
/// or the latest tagged release for stable.
/// With `run_id` it is the artifact of that `devel` push run of CI.
/// Returns the path to the downloaded binary and the temp directory that owns it.
pub fn download(
    update_channel: UpdateChannel,
    run_id: Option<u64>,
) -> anyhow::Result<(Utf8PathBuf, TempDir)> {
    let temp_dir = tempfile::tempdir()?;
    let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("Non-UTF8 temp dir path: {}", path.display()))?;

    let name = if let Some(run_id) = run_id {
        check_devel_push(run_id, &view_run(run_id)?)?;
        let name = binary_name(DEVEL);
        println!("Downloading artifact {name} from devel run {run_id}...");
        gh(&[
            "run",
            "download",
            &run_id.to_string(),
            "--repo",
            REPO,
            "--name",
            &name,
            "--dir",
            temp_path.as_str(),
        ])?;
        name
    } else {
        let tag = release_tag(update_channel, latest_release_tag)?;
        let name = binary_name(&tag);
        println!("Downloading {name} from the {tag} release...");
        gh(&[
            "release",
            "download",
            &tag,
            "--repo",
            REPO,
            "--pattern",
            &name,
            "--pattern",
            &format!("{name}.sha256"),
            "--dir",
            temp_path.as_str(),
        ])?;
        name
    };

    let binary_path = temp_path.join(&name);
    let checksum = verify_checksum(&binary_path, &temp_path.join(format!("{name}.sha256")))?;
    println!("Downloaded runner binary to {binary_path}, sha256 {checksum}");

    Ok((binary_path, temp_dir))
}

/// The sha256 the release of the update channel publishes for its runner binary.
pub fn published_checksum(update_channel: UpdateChannel) -> anyhow::Result<Sha256> {
    let tag = release_tag(update_channel, latest_release_tag)?;
    let name = format!("{}.sha256", binary_name(&tag));
    let published = gh(&[
        "release",
        "download",
        &tag,
        "--repo",
        REPO,
        "--pattern",
        &name,
        "--output",
        "-",
    ])?;
    let published = String::from_utf8_lossy(&published);
    parse_checksum(&published).with_context(|| format!("invalid checksum in {name}: {published}"))
}

fn binary_name(version: &str) -> String {
    format!("runner-{version}-linux-x86-64")
}

fn release_tag(
    update_channel: UpdateChannel,
    latest_release_tag: impl FnOnce() -> anyhow::Result<String>,
) -> anyhow::Result<String> {
    match update_channel {
        UpdateChannel::Canary => Ok(CANARY.to_owned()),
        UpdateChannel::Stable => latest_release_tag(),
    }
}

/// GitHub's latest release is the newest tagged one, never the `canary` prerelease.
fn latest_release_tag() -> anyhow::Result<String> {
    let stdout = gh(&[
        "release", "view", "--repo", REPO, "--json", "tagName", "--jq", ".tagName",
    ])?;
    let tag = String::from_utf8_lossy(&stdout).trim().to_owned();
    anyhow::ensure!(!tag.is_empty(), "gh release view returned no tag");
    Ok(tag)
}

fn view_run(run_id: u64) -> anyhow::Result<RunView> {
    let stdout = gh(&[
        "run",
        "view",
        &run_id.to_string(),
        "--repo",
        REPO,
        "--json",
        "event,headBranch",
    ])?;
    serde_json::from_slice(&stdout).with_context(|| {
        format!(
            "unexpected gh run view output: {}",
            String::from_utf8_lossy(&stdout)
        )
    })
}

/// A pull request from a fork's branch named `devel` uploads artifacts under the same names.
fn check_devel_push(run_id: u64, run: &RunView) -> anyhow::Result<()> {
    let RunView { event, head_branch } = run;
    anyhow::ensure!(
        event == "push" && head_branch == DEVEL,
        "Run {run_id} is a {event} run on {head_branch}; only a push run of {DEVEL} can be deployed"
    );
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunView {
    event: String,
    head_branch: String,
}

fn gh(args: &[&str]) -> anyhow::Result<Vec<u8>> {
    let output = Command::new("gh")
        .args(args)
        .output()
        .context("failed to run gh")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh {} failed: {stderr}", args.join(" "));
    }
    Ok(output.stdout)
}

/// Check the binary against the first field of its `sha256sum` file.
fn verify_checksum(binary: &Utf8Path, checksum_file: &Utf8Path) -> anyhow::Result<Sha256> {
    let published = std::fs::read_to_string(checksum_file)
        .with_context(|| format!("failed to read {checksum_file}"))?;
    let expected = parse_checksum(&published)
        .with_context(|| format!("invalid checksum in {checksum_file}: {published}"))?;
    let contents = std::fs::read(binary).with_context(|| format!("failed to read {binary}"))?;
    let actual: Sha256 = hex::encode(sha2::Sha256::digest(contents)).parse()?;
    anyhow::ensure!(
        actual == expected,
        "{binary} has sha256 {actual}, but {checksum_file} publishes {expected}"
    );
    Ok(expected)
}

/// The first field of a `sha256sum` line.
fn parse_checksum(published: &str) -> anyhow::Result<Sha256> {
    Ok(published
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .parse()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(event: &str, head_branch: &str) -> RunView {
        RunView {
            event: event.to_owned(),
            head_branch: head_branch.to_owned(),
        }
    }

    #[test]
    fn release_tag_follows_the_update_channel() {
        let latest = || anyhow::Ok("v1.2.3".to_owned());
        assert_eq!(release_tag(UpdateChannel::Canary, latest).unwrap(), CANARY);
        assert_eq!(
            release_tag(UpdateChannel::Stable, latest).unwrap(),
            "v1.2.3"
        );
    }

    #[test]
    fn check_devel_push_accepts_only_a_devel_push() {
        check_devel_push(1, &run("push", "devel")).unwrap();
        check_devel_push(1, &run("pull_request", "devel")).unwrap_err();
        check_devel_push(1, &run("push", "cloud")).unwrap_err();
    }

    #[test]
    fn verify_checksum_rejects_a_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let dir = Utf8Path::from_path(dir.path()).unwrap();
        let binary = dir.join("runner");
        let checksum_file = dir.join("runner.sha256");
        std::fs::write(&binary, b"runner").unwrap();
        let published = hex::encode(sha2::Sha256::digest(b"runner"));
        std::fs::write(&checksum_file, format!("{published}  runner\n")).unwrap();
        assert_eq!(
            verify_checksum(&binary, &checksum_file).unwrap().as_ref(),
            published
        );

        std::fs::write(&binary, b"tampered").unwrap();
        verify_checksum(&binary, &checksum_file).unwrap_err();
    }
}
