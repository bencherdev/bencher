use std::io::Read as _;

use anyhow::Context as _;
use sha2::{Digest as _, Sha256};

const S3_BUCKET_URL: &str = "https://s3.amazonaws.com/spec.ccfc.min";

pub struct KernelArtifacts {
    pub url_x86_64: String,
    pub url_aarch64: String,
    pub sha256_x86_64: String,
    pub sha256_aarch64: String,
    pub version: String,
}

pub fn find_latest(pinned_minor: &str) -> anyhow::Result<KernelArtifacts> {
    let build_id = find_newest_ci_build()?;

    let version_x86_64 = find_kernel_version(&build_id, "x86_64", pinned_minor)?;
    let version_aarch64 = find_kernel_version(&build_id, "aarch64", pinned_minor)?;

    anyhow::ensure!(
        version_x86_64 == version_aarch64,
        "kernel version mismatch: x86_64 has {version_x86_64} but aarch64 has {version_aarch64}"
    );

    let version = version_x86_64;

    let url_x86_64 = format!("{S3_BUCKET_URL}/firecracker-ci/{build_id}/x86_64/vmlinux-{version}");
    let url_aarch64 =
        format!("{S3_BUCKET_URL}/firecracker-ci/{build_id}/aarch64/vmlinux-{version}");

    let sha256_x86_64 = download_and_hash(&url_x86_64).context("failed to hash x86_64 kernel")?;
    let sha256_aarch64 =
        download_and_hash(&url_aarch64).context("failed to hash aarch64 kernel")?;

    Ok(KernelArtifacts {
        url_x86_64,
        url_aarch64,
        sha256_x86_64,
        sha256_aarch64,
        version,
    })
}

fn find_newest_ci_build() -> anyhow::Result<String> {
    let url = format!("{S3_BUCKET_URL}?list-type=2&prefix=firecracker-ci/&delimiter=/");

    let xml = ureq::get(&url)
        .call()
        .context("failed to list S3 CI builds")?
        .body_mut()
        .read_to_string()
        .context("failed to read S3 listing")?;

    let prefixes = extract_xml_values(&xml, "Prefix");

    // Filter to date-based directories: firecracker-ci/YYYYMMDD-{hash}-{n}/
    // The name must start with exactly 8 digits (a date) followed by a hyphen.
    let mut date_dirs: Vec<&str> = prefixes
        .iter()
        .filter_map(|p| {
            let name = p.strip_prefix("firecracker-ci/")?.strip_suffix('/')?;
            let is_date_dir = name.len() > 9
                && name.as_bytes().iter().take(8).all(u8::is_ascii_digit)
                && name.as_bytes().get(8) == Some(&b'-');
            is_date_dir.then_some(name)
        })
        .collect();

    date_dirs.sort_unstable();

    let newest = date_dirs
        .last()
        .context("no date-based CI build directories found in S3")?;

    Ok((*newest).to_owned())
}

fn find_kernel_version(build_id: &str, arch: &str, pinned_minor: &str) -> anyhow::Result<String> {
    let prefix = format!("firecracker-ci/{build_id}/{arch}/vmlinux-");
    let url = format!("{S3_BUCKET_URL}?list-type=2&prefix={prefix}");

    let xml = ureq::get(&url)
        .call()
        .with_context(|| format!("failed to list kernels for {arch}"))?
        .body_mut()
        .read_to_string()
        .with_context(|| format!("failed to read kernel listing for {arch}"))?;

    let keys = extract_xml_values(&xml, "Key");
    let version_prefix = format!("{pinned_minor}.");

    let best_version = keys
        .iter()
        .filter_map(|key| {
            let filename = key.rsplit('/').next()?;
            // Skip .config and -no-acpi variants
            if filename.contains(".config") || filename.contains("-no-acpi") {
                return None;
            }
            let version = filename.strip_prefix("vmlinux-")?;
            version
                .starts_with(&version_prefix)
                .then(|| version.to_owned())
        })
        .max_by(|a, b| compare_versions(a, b))
        .with_context(|| {
            format!("no kernel matching {pinned_minor}.x found in {build_id}/{arch}")
        })?;

    Ok(best_version)
}

fn download_and_hash(url: &str) -> anyhow::Result<String> {
    let mut reader = ureq::get(url)
        .call()
        .with_context(|| format!("failed to download {url}"))?
        .into_body()
        .into_reader();

    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = reader
            .read(&mut buf)
            .context("failed to read kernel body")?;
        if n == 0 {
            break;
        }
        hasher.update(buf.get(..n).context("buffer slice out of bounds")?);
    }

    let hash = hasher.finalize();
    Ok(format!("{hash:x}"))
}

fn extract_xml_values(xml: &str, tag: &str) -> Vec<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut values = Vec::new();
    let mut remaining = xml;
    while let Some(start) = remaining.find(&open) {
        let after_open = start + open.len();
        let value_str = remaining.get(after_open..).unwrap_or_default();
        if let Some(end) = value_str.find(&close) {
            let value = value_str.get(..end).unwrap_or_default();
            values.push(value.to_owned());
            remaining = value_str.get(end + close.len()..).unwrap_or_default();
        } else {
            break;
        }
    }
    values
}

fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> Vec<u64> { v.split('.').filter_map(|s| s.parse().ok()).collect() };
    parse(a).cmp(&parse(b))
}
