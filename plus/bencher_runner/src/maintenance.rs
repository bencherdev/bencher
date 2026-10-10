//! The names through which host maintenance and the runner's Jobs take turns,
//! shared with the drop-ins that have a maintenance unit wait its turn.

use std::path::Path;

/// The runner's locks and the maintenance markers.
pub const RUN_DIR: &str = "/run/bencher";
/// The lock the runner holds through each Job, in [`RUN_DIR`].
pub const JOB_LOCK: &str = "job.lock";
/// A marker is a file in [`RUN_DIR`] named this, or this then `.<name>`.
pub const MARKER: &str = "maintenance";

/// Whether a maintenance marker is present in `dir`.
pub fn marker_present(dir: &Path) -> bool {
    dir.read_dir().is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            entry.file_name().to_str().is_some_and(|name| {
                name == MARKER
                    || name
                        .strip_prefix(MARKER)
                        .is_some_and(|rest| rest.starts_with('.'))
            })
        })
    })
}
