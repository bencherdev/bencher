use std::process::Command;

use camino::Utf8PathBuf;
use tempfile::TempDir;

const ARTIFACT_NAME: &str = "runner-cloud-linux-x86-64";

/// Download the runner binary from GitHub Actions.
///
/// If `run_id` is `None`, finds the latest successful `cloud` branch run.
/// Returns the path to the downloaded binary and the temp directory that owns it.
pub fn download(run_id: Option<u64>) -> anyhow::Result<(Utf8PathBuf, TempDir)> {
    let run_id = match run_id {
        Some(id) => id,
        None => latest_cloud_run_id()?,
    };

    println!("Downloading runner artifact from run {run_id}...");

    let temp_dir = tempfile::tempdir()?;
    let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("Non-UTF8 temp dir path: {}", path.display()))?;

    let output = Command::new("gh")
        .args([
            "run",
            "download",
            &run_id.to_string(),
            "--repo",
            "bencherdev/bencher",
            "-n",
            ARTIFACT_NAME,
            "-D",
            temp_path.as_str(),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh run download failed: {stderr}");
    }

    let binary_path = temp_path.join(ARTIFACT_NAME);
    if !binary_path.exists() {
        anyhow::bail!(
            "Expected binary not found at {binary_path}; contents: {:?}",
            std::fs::read_dir(&temp_path)?
                .filter_map(|e| e.ok().map(|e| e.file_name()))
                .collect::<Vec<_>>()
        );
    }

    println!("Downloaded runner binary to {binary_path}");

    Ok((binary_path, temp_dir))
}

fn latest_cloud_run_id() -> anyhow::Result<u64> {
    println!("Finding latest successful cloud CI run...");

    let output = Command::new("gh")
        .args([
            "run",
            "list",
            "--repo",
            "bencherdev/bencher",
            "--branch",
            "cloud",
            "--workflow",
            "ci.yml",
            "--status",
            "success",
            "--json",
            "databaseId",
            "-L",
            "1",
            "--jq",
            ".[0].databaseId",
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh run list failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let run_id: u64 = stdout
        .trim()
        .parse()
        .map_err(|e| anyhow::anyhow!("Failed to parse run ID from '{stdout}': {e}"))?;

    println!("Latest cloud run ID: {run_id}");
    Ok(run_id)
}
