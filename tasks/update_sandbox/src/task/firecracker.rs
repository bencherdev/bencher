use anyhow::Context as _;
use serde::Deserialize;

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    prerelease: bool,
}

pub struct FirecrackerRelease {
    pub tag: String,
    pub sha256_x86_64: String,
    pub sha256_aarch64: String,
}

pub fn find_latest(pinned_minor: &str) -> anyhow::Result<FirecrackerRelease> {
    let prefix = format!("v{pinned_minor}.");

    let mut request = ureq::get(
        "https://api.github.com/repos/firecracker-microvm/firecracker/releases?per_page=100",
    )
    .header("Accept", "application/vnd.github+json")
    .header("User-Agent", "bencher-update-sandbox");

    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }

    let body = request
        .call()
        .context("failed to fetch Firecracker releases")?
        .body_mut()
        .read_to_string()
        .context("failed to read releases body")?;

    let releases: Vec<Release> =
        serde_json::from_str(&body).context("failed to parse releases JSON")?;

    let best = releases
        .iter()
        .filter(|r| !r.prerelease && r.tag_name.starts_with(&prefix))
        .max_by(|a, b| compare_tags(&a.tag_name, &b.tag_name))
        .with_context(|| format!("no Firecracker release found matching v{pinned_minor}.x"))?;

    let tag = best.tag_name.clone();

    let sha256_x86_64 = download_sha256(&tag, "x86_64")?;
    let sha256_aarch64 = download_sha256(&tag, "aarch64")?;

    Ok(FirecrackerRelease {
        tag,
        sha256_x86_64,
        sha256_aarch64,
    })
}

fn download_sha256(tag: &str, arch: &str) -> anyhow::Result<String> {
    let url = format!(
        "https://github.com/firecracker-microvm/firecracker/releases/download/{tag}/firecracker-{tag}-{arch}.tgz.sha256.txt"
    );

    let body = ureq::get(&url)
        .header("User-Agent", "bencher-update-sandbox")
        .call()
        .with_context(|| format!("failed to download SHA256 for {tag} {arch}"))?
        .body_mut()
        .read_to_string()
        .with_context(|| format!("failed to read SHA256 body for {tag} {arch}"))?;

    // Format: "{hash}  {filename}\n"
    let hash = body
        .split_whitespace()
        .next()
        .with_context(|| format!("empty SHA256 file for {tag} {arch}"))?;

    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("invalid SHA256 hash for {tag} {arch}: {hash}");
    }

    Ok(hash.to_owned())
}

fn compare_tags(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |tag: &str| -> Vec<u64> {
        tag.trim_start_matches('v')
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect()
    };
    parse(a).cmp(&parse(b))
}
