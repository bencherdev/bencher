use std::fs;

use anyhow::Context as _;
use camino::Utf8Path;

pub struct BuildRsValues {
    pub firecracker_version: String,
    pub firecracker_sha256_x86_64: String,
    pub firecracker_sha256_aarch64: String,
    pub kernel_url_x86_64: String,
    pub kernel_url_aarch64: String,
    pub kernel_sha256_x86_64: String,
    pub kernel_sha256_aarch64: String,
}

pub fn extract_const(content: &str, name: &str) -> anyhow::Result<String> {
    let prefix = format!("const {name}: &str =");
    let mut lines = content.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if !trimmed.starts_with(&prefix) {
            continue;
        }
        // Single-line: const X: &str = "value";
        if trimmed.contains('"') {
            return extract_quoted_value(trimmed);
        }
        // Multi-line: the value is on the next line
        if let Some(next_line) = lines.next() {
            return extract_quoted_value(next_line.trim());
        }
    }
    anyhow::bail!("constant {name} not found in build.rs");
}

fn extract_quoted_value(s: &str) -> anyhow::Result<String> {
    let start = s
        .find('"')
        .with_context(|| format!("no opening quote in: {s}"))?;
    let after_quote = s.get(start + 1..).context("unexpected end of string")?;
    let end = after_quote
        .find('"')
        .with_context(|| format!("no closing quote in: {s}"))?;
    Ok(after_quote
        .get(..end)
        .context("unexpected end of string")?
        .to_owned())
}

pub fn apply_updates(
    build_rs_path: &Utf8Path,
    old: &BuildRsValues,
    new: &BuildRsValues,
) -> anyhow::Result<bool> {
    let content = fs::read_to_string(build_rs_path)
        .with_context(|| format!("failed to read {build_rs_path}"))?;

    let replacements = [
        (
            "DEFAULT_FIRECRACKER_VERSION",
            &old.firecracker_version,
            &new.firecracker_version,
        ),
        (
            "FIRECRACKER_TGZ_SHA256_X86_64",
            &old.firecracker_sha256_x86_64,
            &new.firecracker_sha256_x86_64,
        ),
        (
            "FIRECRACKER_TGZ_SHA256_AARCH64",
            &old.firecracker_sha256_aarch64,
            &new.firecracker_sha256_aarch64,
        ),
        (
            "DEFAULT_KERNEL_URL_X86_64",
            &old.kernel_url_x86_64,
            &new.kernel_url_x86_64,
        ),
        (
            "DEFAULT_KERNEL_URL_AARCH64",
            &old.kernel_url_aarch64,
            &new.kernel_url_aarch64,
        ),
        (
            "KERNEL_SHA256_X86_64",
            &old.kernel_sha256_x86_64,
            &new.kernel_sha256_x86_64,
        ),
        (
            "KERNEL_SHA256_AARCH64",
            &old.kernel_sha256_aarch64,
            &new.kernel_sha256_aarch64,
        ),
    ];

    let mut updated = content.clone();
    for (name, old_val, new_val) in replacements {
        updated = replace_const(&updated, name, old_val, new_val)?;
    }

    if updated == content {
        return Ok(false);
    }

    fs::write(build_rs_path, &updated)
        .with_context(|| format!("failed to write {build_rs_path}"))?;

    Ok(true)
}

fn replace_const(
    content: &str,
    name: &str,
    old_value: &str,
    new_value: &str,
) -> anyhow::Result<String> {
    let prefix = format!("const {name}: &str =");
    let mut result = String::with_capacity(content.len());
    let mut lines = content.lines();
    let mut found = false;

    while let Some(line) = lines.next() {
        if !found && line.trim().starts_with(&prefix) {
            found = true;
            if line.contains(old_value) {
                result.push_str(&line.replacen(old_value, new_value, 1));
                result.push('\n');
            } else {
                result.push_str(line);
                result.push('\n');
                // Value is on the next line
                if let Some(next_line) = lines.next() {
                    result.push_str(&next_line.replacen(old_value, new_value, 1));
                    result.push('\n');
                }
            }
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }

    if !found {
        anyhow::bail!("constant {name} not found in build.rs");
    }

    // Preserve original trailing newline behavior
    if !content.ends_with('\n') {
        result.pop();
    }

    Ok(result)
}

pub fn read_current(build_rs_path: &Utf8Path) -> anyhow::Result<BuildRsValues> {
    let content = fs::read_to_string(build_rs_path)
        .with_context(|| format!("failed to read {build_rs_path}"))?;

    Ok(BuildRsValues {
        firecracker_version: extract_const(&content, "DEFAULT_FIRECRACKER_VERSION")?,
        firecracker_sha256_x86_64: extract_const(&content, "FIRECRACKER_TGZ_SHA256_X86_64")?,
        firecracker_sha256_aarch64: extract_const(&content, "FIRECRACKER_TGZ_SHA256_AARCH64")?,
        kernel_url_x86_64: extract_const(&content, "DEFAULT_KERNEL_URL_X86_64")?,
        kernel_url_aarch64: extract_const(&content, "DEFAULT_KERNEL_URL_AARCH64")?,
        kernel_sha256_x86_64: extract_const(&content, "KERNEL_SHA256_X86_64")?,
        kernel_sha256_aarch64: extract_const(&content, "KERNEL_SHA256_AARCH64")?,
    })
}
