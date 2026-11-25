use std::time::Instant;

use camino::Utf8Path;
use slog::{Logger, info};

use crate::error::ExecutionError;

/// Writes back every dirty page of each filesystem that holds one of
/// `written`, and waits for it, so the kernel's writeback of what a run's
/// preparation wrote up to here does not run during the run. What is written
/// after it is not covered: on the VM path, the jailer's copy of Firecracker
/// and the VM's config, a few MiB the same for every run. The pages stay in
/// the page cache, clean, so the run reads them as warm as before.
pub(crate) fn flush_filesystems(log: &Logger, written: &[&Utf8Path]) -> Result<(), ExecutionError> {
    let started = Instant::now();
    for path in written {
        syncfs(path)?;
    }
    info!(log, "Flushed the run's files to disk";
        "flush_ms" => u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    );
    Ok(())
}

fn syncfs(path: &Utf8Path) -> Result<(), ExecutionError> {
    let flush_error = |source| ExecutionError::Flush {
        path: path.to_owned(),
        source,
    };
    let file = std::fs::File::open(path).map_err(flush_error)?;
    nix::unistd::syncfs(&file).map_err(|errno| flush_error(errno.into()))
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use super::*;
    use crate::log::discard;

    #[test]
    fn a_path_that_cannot_be_flushed_fails_naming_it() {
        // Kills a flush that drops a path's error, which would boot a VM on
        // files whose writeback is still to come.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let missing = root.join("missing");

        let result = flush_filesystems(&discard(), &[&root, &missing]);

        let Err(ExecutionError::Flush { path, .. }) = &result else {
            panic!("a path that cannot be flushed must fail the flush, got: {result:?}");
        };
        assert_eq!(path, &missing);
    }
}
