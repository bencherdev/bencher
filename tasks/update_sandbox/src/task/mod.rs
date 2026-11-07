use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser as _;

use crate::parser::TaskUpdateSandbox;

mod build_rs;
mod firecracker;
mod kernel;

use build_rs::BuildRsValues;

#[derive(Debug)]
pub struct Task {
    dry_run: bool,
    build_rs_path: Utf8PathBuf,
    firecracker_version: Option<String>,
    newer_major_out: Option<Utf8PathBuf>,
    kernel_version: String,
}

impl TryFrom<TaskUpdateSandbox> for Task {
    type Error = anyhow::Error;

    fn try_from(parser: TaskUpdateSandbox) -> Result<Self, Self::Error> {
        let TaskUpdateSandbox {
            dry_run,
            build_rs,
            firecracker_version,
            newer_major_out,
            kernel_version,
        } = parser;

        let build_rs_path = match build_rs {
            Some(path) => path,
            None => default_build_rs_path(),
        };

        Ok(Self {
            dry_run,
            build_rs_path,
            firecracker_version,
            newer_major_out,
            kernel_version,
        })
    }
}

impl Task {
    pub fn new() -> anyhow::Result<Self> {
        TaskUpdateSandbox::parse().try_into()
    }

    pub fn exec(&self) -> anyhow::Result<()> {
        let current = build_rs::read_current(&self.build_rs_path)?;

        let line = firecracker::Line::new(
            &current.firecracker_version,
            self.firecracker_version.as_deref(),
        )?;
        let firecracker::Selection { tag, newer_major } = firecracker::latest(line)?;

        let notice = newer_major.map(|newer_major| newer_major.notice(line));
        // Not stdout: the workflow takes that for the commit message and the PR title.
        if let Some(notice) = &notice {
            eprintln!("{notice}");
        }
        if let Some(path) = &self.newer_major_out {
            write_newer_major(path, notice.as_deref())?;
        }

        let firecracker::Checksums { x86_64, aarch64 } = firecracker::checksums(&tag)?;
        let kern = kernel::find_latest(&self.kernel_version)?;

        let new = BuildRsValues {
            firecracker_version: tag,
            firecracker_sha256_x86_64: x86_64,
            firecracker_sha256_aarch64: aarch64,
            kernel_url_x86_64: kern.url_x86_64,
            kernel_url_aarch64: kern.url_aarch64,
            kernel_sha256_x86_64: kern.sha256_x86_64,
            kernel_sha256_aarch64: kern.sha256_aarch64,
        };

        let fc_changed = current.firecracker_version != new.firecracker_version;
        let current_kernel_version = current
            .kernel_url_x86_64
            .rsplit('/')
            .next()
            .and_then(|f| f.strip_prefix("vmlinux-"))
            .context("failed to parse kernel version from current URL")?;
        let kern_changed = current_kernel_version != kern.version;

        if !fc_changed && !kern_changed {
            println!("Already up to date");
            return Ok(());
        }

        let summary = format_summary(
            &current,
            &new,
            current_kernel_version,
            &kern.version,
            fc_changed,
            kern_changed,
        );

        if self.dry_run {
            println!("[dry run] {summary}");
            return Ok(());
        }

        build_rs::apply_updates(&self.build_rs_path, &current, &new)?;
        println!("{summary}");

        Ok(())
    }
}

/// A notice has no reader until the task ends, so hand it over as a file:
/// the workflow fails the run on a notice, after it opens the pull request.
fn write_newer_major(path: &Utf8Path, notice: Option<&str>) -> anyhow::Result<()> {
    let contents = notice.map_or_else(String::new, |notice| format!("{notice}\n"));
    std::fs::write(path, contents)
        .with_context(|| format!("failed to write the newer major notice to `{path}`"))
}

fn format_summary(
    current: &BuildRsValues,
    new: &BuildRsValues,
    current_kernel_version: &str,
    new_kernel_version: &str,
    fc_changed: bool,
    kern_changed: bool,
) -> String {
    let mut parts = Vec::new();

    if fc_changed {
        parts.push(format!(
            "Firecracker {} -> {}",
            current.firecracker_version, new.firecracker_version
        ));
    }

    if kern_changed {
        parts.push(format!(
            "kernel {current_kernel_version} -> {new_kernel_version}"
        ));
    }

    format!("Update {}", parts.join(", "))
}

fn default_build_rs_path() -> Utf8PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_owned());
    let manifest_path = Utf8PathBuf::from(manifest_dir);

    // Walk up to find the workspace root (contains Cargo.lock)
    let mut workspace_root = manifest_path;
    while let Some(parent) = workspace_root.parent() {
        if workspace_root.join("Cargo.lock").exists() {
            break;
        }
        workspace_root = parent.to_path_buf();
    }

    workspace_root.join("plus/bencher_runner/build.rs")
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use super::write_newer_major;

    /// The workflow reads the notice from the file and fails the run on it, so
    /// a run without one has to leave nothing behind for the next to find.
    #[test]
    fn a_notice_is_handed_over_whole_and_taken_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().join("newer-major.txt")).unwrap();

        write_newer_major(&path, Some("Firecracker v2.0.0 is out")).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "Firecracker v2.0.0 is out\n"
        );

        write_newer_major(&path, None).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
    }
}
