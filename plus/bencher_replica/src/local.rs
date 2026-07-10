//! Local-filesystem replica backend.
//!
//! Objects live under a root directory; keys map to relative paths. Writes
//! land in a `<final>.partial-<uuid>` sibling then rename into place (atomic
//! visibility), with the file fsynced before the rename (matching the
//! `rate_limiting.json` convention in `bencher_schema`). The parent directory
//! is fsynced after the rename, and each newly created parent directory is
//! fsynced as it is made, so an acknowledged `put` is durable across a power
//! loss (not merely atomically visible). A `put` that fails removes its own
//! partial file so a crash-looping writer cannot accumulate them. Because
//! partial files live inside the object tree, `list` filters out any file
//! whose name contains the partial infix; replica keys never contain it.

use std::io;

use bytes::Bytes;
use camino::{Utf8Path, Utf8PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt as _;
use uuid::Uuid;

use crate::storage::StorageError;

/// The infix separating a final object name from its temp-write suffix.
/// Files containing it are in-progress writes, never objects.
const PARTIAL_INFIX: &str = ".partial-";

/// Local filesystem backend rooted at a directory.
#[derive(Debug, Clone)]
pub struct LocalStorage {
    root: Utf8PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum LocalError {
    #[error("Failed to create directory ({path}): {error}")]
    CreateDir { path: Utf8PathBuf, error: io::Error },
    #[error("Failed to read ({path}): {error}")]
    Read { path: Utf8PathBuf, error: io::Error },
    #[error("Failed to write ({path}): {error}")]
    Write { path: Utf8PathBuf, error: io::Error },
    #[error("Failed to rename ({from} -> {to}): {error}")]
    Rename {
        from: Utf8PathBuf,
        to: Utf8PathBuf,
        error: io::Error,
    },
    #[error("Failed to remove ({path}): {error}")]
    Remove { path: Utf8PathBuf, error: io::Error },
    #[error("Failed to list ({path}): {error}")]
    List { path: Utf8PathBuf, error: io::Error },
    #[error("Non-UTF-8 path in replica directory: {0}")]
    NonUtf8Path(std::path::PathBuf),
}

impl LocalStorage {
    #[must_use]
    pub fn new(root: Utf8PathBuf) -> Self {
        Self { root }
    }

    #[must_use]
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }

    pub(crate) async fn put(&self, key: &str, bytes: Bytes) -> Result<(), StorageError> {
        crate::storage::validate_key(key)?;
        let final_path = self.root.join(key);
        create_parent_dirs(&final_path).await?;
        let partial_path = partial_path_for(&final_path);
        // On any failure, remove the partial so a crash-looping writer cannot
        // accumulate unbounded `.partial-` files; the original error wins.
        match write_then_rename(&partial_path, &final_path, &bytes).await {
            Ok(()) => Ok(()),
            Err(error) => {
                remove_partial(&partial_path).await;
                Err(error.into())
            },
        }
    }

    pub(crate) async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
        crate::storage::validate_key(key)?;
        let path = self.root.join(key);
        match fs::read(&path).await {
            Ok(bytes) => Ok(Bytes::from(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Err(StorageError::NotFound {
                key: key.to_owned(),
            }),
            Err(error) => Err(LocalError::Read { path, error }.into()),
        }
    }

    pub(crate) async fn get_stream(
        &self,
        key: &str,
    ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, StorageError> {
        crate::storage::validate_key(key)?;
        let path = self.root.join(key);
        match fs::File::open(&path).await {
            Ok(file) => Ok(Box::new(file)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Err(StorageError::NotFound {
                key: key.to_owned(),
            }),
            Err(error) => Err(LocalError::Read { path, error }.into()),
        }
    }

    pub(crate) async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        // Start the walk at the deepest directory implied by the prefix so
        // unrelated trees are not scanned; filter by full key prefix below
        // (S3 semantics: the prefix may end mid-component).
        let start_dir = match prefix.rsplit_once('/') {
            Some((dir, _)) => self.root.join(dir),
            None => self.root.clone(),
        };
        // Iterative walk with an explicit stack: async fns cannot recurse
        // without boxing.
        let mut keys = Vec::new();
        let mut stack = vec![start_dir];
        while let Some(dir) = stack.pop() {
            let mut entries = match fs::read_dir(&dir).await {
                Ok(entries) => entries,
                // A missing prefix directory is an empty result, not an error.
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(LocalError::List { path: dir, error }.into()),
            };
            while let Some(entry) =
                entries
                    .next_entry()
                    .await
                    .map_err(|error| LocalError::List {
                        path: dir.clone(),
                        error,
                    })?
            {
                let path =
                    Utf8PathBuf::from_path_buf(entry.path()).map_err(LocalError::NonUtf8Path)?;
                let file_type = entry.file_type().await.map_err(|error| LocalError::List {
                    path: path.clone(),
                    error,
                })?;
                if file_type.is_dir() {
                    stack.push(path);
                } else if let Ok(relative) = path.strip_prefix(&self.root) {
                    let key = relative_key(relative);
                    if key.starts_with(prefix) && !key.contains(PARTIAL_INFIX) {
                        keys.push(key);
                    }
                }
            }
        }
        keys.sort();
        Ok(keys)
    }

    /// Best-effort reaper for `.partial-` fragments orphaned by a crash
    /// (SIGKILL/OOM) mid-write: the error-path cleanup in [`Self::put`] and
    /// the multipart finish never ran, so a potentially multi-GB fragment
    /// lingers invisibly (listings filter the infix) forever. Safe at
    /// startup: exactly one writer per target (see the crate operational
    /// assumptions), so no in-flight write can race the sweep. Walk errors
    /// are logged and swallowed; this never fails the caller.
    pub(crate) async fn abort_incomplete_uploads(&self, log: &slog::Logger) {
        let mut removed = 0u64;
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            let mut entries = match fs::read_dir(&dir).await {
                Ok(entries) => entries,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => {
                    slog::warn!(log, "Failed to read directory during partial-file sweep";
                        "path" => dir.as_str(), "error" => %error);
                    continue;
                },
            };
            loop {
                let entry = match entries.next_entry().await {
                    Ok(Some(entry)) => entry,
                    Ok(None) => break,
                    Err(error) => {
                        slog::warn!(log, "Failed to read directory entry during partial-file sweep";
                            "path" => dir.as_str(), "error" => %error);
                        break;
                    },
                };
                let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
                    continue;
                };
                let is_dir = entry
                    .file_type()
                    .await
                    .is_ok_and(|file_type| file_type.is_dir());
                if is_dir {
                    stack.push(path);
                } else if path
                    .file_name()
                    .is_some_and(|name| name.contains(PARTIAL_INFIX))
                {
                    match fs::remove_file(&path).await {
                        Ok(()) => {
                            removed += 1;
                            slog::info!(log, "Removed crash-orphaned partial file";
                                "path" => path.as_str());
                        },
                        Err(error) => {
                            slog::warn!(log, "Failed to remove crash-orphaned partial file";
                                "path" => path.as_str(), "error" => %error);
                        },
                    }
                }
            }
        }
        if removed > 0 {
            slog::info!(log, "Partial-file sweep complete"; "removed" => removed);
        }
    }

    pub(crate) async fn list_dirs(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        let dir = if prefix.is_empty() {
            self.root.clone()
        } else {
            self.root.join(prefix)
        };
        let mut entries = match fs::read_dir(&dir).await {
            Ok(entries) => entries,
            // A missing prefix directory is an empty result, not an error.
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(LocalError::List { path: dir, error }.into()),
        };
        let mut dirs = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|error| LocalError::List {
                path: dir.clone(),
                error,
            })?
        {
            let file_type = entry.file_type().await.map_err(|error| LocalError::List {
                path: dir.clone(),
                error,
            })?;
            if file_type.is_dir() {
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|name| LocalError::NonUtf8Path(name.into()))?;
                dirs.push(name);
            }
        }
        dirs.sort();
        Ok(dirs)
    }

    pub(crate) async fn delete(&self, key: &str) -> Result<(), StorageError> {
        crate::storage::validate_key(key)?;
        let path = self.root.join(key);
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            // Idempotent: deleting a missing key succeeds.
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(LocalError::Remove { path, error }.into()),
        }
    }

    pub(crate) async fn delete_prefix(&self, prefix: &str) -> Result<(), StorageError> {
        // The local backend treats the prefix as a directory path; replica
        // pruning always deletes directory-aligned prefixes (generations).
        if prefix.is_empty() {
            // Delete the root's children, never the root itself, so the
            // backend stays usable afterwards.
            return self.delete_root_children().await;
        }
        let path = self.root.join(prefix);
        match fs::remove_dir_all(&path).await {
            Ok(()) => Ok(()),
            // A missing prefix is already deleted.
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(LocalError::Remove { path, error }.into()),
        }
    }

    pub(crate) async fn start_multipart(&self, key: &str) -> Result<LocalMultipart, StorageError> {
        crate::storage::validate_key(key)?;
        let final_path = self.root.join(key);
        create_parent_dirs(&final_path).await?;
        let partial_path = partial_path_for(&final_path);
        let file = fs::File::create(&partial_path)
            .await
            .map_err(|error| LocalError::Write {
                path: partial_path.clone(),
                error,
            })?;
        Ok(LocalMultipart {
            final_path,
            partial_path,
            file,
        })
    }

    async fn delete_root_children(&self) -> Result<(), StorageError> {
        let mut entries = match fs::read_dir(&self.root).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(LocalError::List {
                    path: self.root.clone(),
                    error,
                }
                .into());
            },
        };
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|error| LocalError::List {
                path: self.root.clone(),
                error,
            })?
        {
            let path = Utf8PathBuf::from_path_buf(entry.path()).map_err(LocalError::NonUtf8Path)?;
            let file_type = entry.file_type().await.map_err(|error| LocalError::List {
                path: path.clone(),
                error,
            })?;
            let removed = if file_type.is_dir() {
                fs::remove_dir_all(&path).await
            } else {
                fs::remove_file(&path).await
            };
            match removed {
                Ok(()) => {},
                Err(error) if error.kind() == io::ErrorKind::NotFound => {},
                Err(error) => return Err(LocalError::Remove { path, error }.into()),
            }
        }
        Ok(())
    }
}

/// Streaming upload to the local backend: writes to `<final>.partial-<uuid>`,
/// fsyncs, then renames on finish. Dropping without finish leaves only the
/// partial file, never the final name; `list` never reports partial files.
pub struct LocalMultipart {
    final_path: Utf8PathBuf,
    partial_path: Utf8PathBuf,
    file: fs::File,
}

impl LocalMultipart {
    pub(crate) async fn write_part(&mut self, bytes: Bytes) -> Result<(), StorageError> {
        self.file
            .write_all(&bytes)
            .await
            .map_err(|error| LocalError::Write {
                path: self.partial_path.clone(),
                error,
            })?;
        Ok(())
    }

    pub(crate) async fn finish(self) -> Result<(), StorageError> {
        let Self {
            final_path,
            partial_path,
            file,
        } = self;
        file.sync_all().await.map_err(|error| LocalError::Write {
            path: partial_path.clone(),
            error,
        })?;
        drop(file);
        rename_into_place(&partial_path, &final_path).await?;
        fsync_parent(&final_path).await?;
        Ok(())
    }

    pub(crate) async fn abort(self) -> Result<(), StorageError> {
        let Self {
            final_path: _,
            partial_path,
            file,
        } = self;
        drop(file);
        match fs::remove_file(&partial_path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(LocalError::Remove {
                path: partial_path,
                error,
            }
            .into()),
        }
    }
}

/// The unique in-progress sibling for a final path: `<final>.partial-<uuid>`.
fn partial_path_for(final_path: &Utf8Path) -> Utf8PathBuf {
    let mut name = final_path.to_string();
    name.push_str(PARTIAL_INFIX);
    name.push_str(&Uuid::new_v4().simple().to_string());
    Utf8PathBuf::from(name)
}

/// Create every missing parent directory of `path`, fsyncing the containing
/// directory after each new level appears. Object durability depends on the
/// parent directories surviving a crash, so the directory entries must be
/// flushed, not just the file content.
async fn create_parent_dirs(path: &Utf8Path) -> Result<(), LocalError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    // Walk up to the nearest existing ancestor, recording the missing levels.
    let mut missing = Vec::new();
    let mut cursor = Some(parent);
    while let Some(dir) = cursor {
        match fs::try_exists(dir).await {
            Ok(true) => break,
            Ok(false) => {
                missing.push(dir.to_owned());
                cursor = dir.parent();
            },
            Err(error) => {
                return Err(LocalError::CreateDir {
                    path: dir.to_owned(),
                    error,
                });
            },
        }
    }
    // Create shallow-to-deep, fsyncing each new directory's container so the
    // new entry is durable.
    for dir in missing.iter().rev() {
        match fs::create_dir(dir).await {
            Ok(()) => {},
            // A concurrent put may have created it first.
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {},
            Err(error) => {
                return Err(LocalError::CreateDir {
                    path: dir.clone(),
                    error,
                });
            },
        }
        fsync_parent(dir).await?;
    }
    Ok(())
}

/// Write to the partial path, rename it into place, and fsync the parent
/// directory so the rename is durable. The single fallible region a failing
/// `put` guards with partial-file cleanup.
async fn write_then_rename(
    partial: &Utf8Path,
    final_path: &Utf8Path,
    bytes: &[u8],
) -> Result<(), LocalError> {
    write_file(partial, bytes).await?;
    rename_into_place(partial, final_path).await?;
    fsync_parent(final_path).await?;
    Ok(())
}

/// Best-effort removal of a partial file on a `put` error path. A failure
/// here is ignored so the original error is preserved; the leftover partial
/// is inert either way (`list` filters the partial infix).
async fn remove_partial(partial: &Utf8Path) {
    drop(fs::remove_file(partial).await);
}

/// Fsync the parent directory of `path` so a rename or child creation within
/// it is durable (the directory metadata is only guaranteed on disk after the
/// containing directory is fsynced; see `restore::finalize` for the same
/// technique).
async fn fsync_parent(path: &Utf8Path) -> Result<(), LocalError> {
    if let Some(parent) = path.parent() {
        fsync_dir(parent).await?;
    }
    Ok(())
}

/// Open a directory and fsync it, flushing its entries to disk.
async fn fsync_dir(dir: &Utf8Path) -> Result<(), LocalError> {
    let handle = fs::File::open(dir)
        .await
        .map_err(|error| LocalError::Read {
            path: dir.to_owned(),
            error,
        })?;
    handle.sync_all().await.map_err(|error| LocalError::Write {
        path: dir.to_owned(),
        error,
    })?;
    Ok(())
}

/// Write `bytes` to `path` and fsync so the content is durable before the
/// atomic rename (see `bencher_schema::context::rate_limiting` for the
/// convention).
async fn write_file(path: &Utf8Path, bytes: &[u8]) -> Result<(), LocalError> {
    let mut file = fs::File::create(path)
        .await
        .map_err(|error| LocalError::Write {
            path: path.to_owned(),
            error,
        })?;
    file.write_all(bytes)
        .await
        .map_err(|error| LocalError::Write {
            path: path.to_owned(),
            error,
        })?;
    file.sync_all().await.map_err(|error| LocalError::Write {
        path: path.to_owned(),
        error,
    })?;
    Ok(())
}

async fn rename_into_place(from: &Utf8Path, to: &Utf8Path) -> Result<(), LocalError> {
    fs::rename(from, to)
        .await
        .map_err(|error| LocalError::Rename {
            from: from.to_owned(),
            to: to.to_owned(),
            error,
        })
}

/// A storage key from a root-relative path: components joined with `/`.
fn relative_key(relative: &Utf8Path) -> String {
    relative
        .components()
        .map(|component| component.as_str())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn storage(tmp: &tempfile::TempDir) -> LocalStorage {
        let root = Utf8Path::from_path(tmp.path())
            .expect("tempdir path is UTF-8")
            .to_path_buf();
        LocalStorage::new(root)
    }

    /// Every file in the tree under `root`, as root-relative strings,
    /// including partial files (unlike `list`).
    fn all_files(root: &Utf8Path) -> Vec<String> {
        let mut files = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("read_dir failed") {
                let path = Utf8PathBuf::from_path_buf(entry.expect("entry failed").path())
                    .expect("path is UTF-8");
                if path.is_dir() {
                    stack.push(path);
                } else {
                    files.push(relative_key(path.strip_prefix(root).expect("under root")));
                }
            }
        }
        files.sort();
        files
    }

    #[tokio::test]
    async fn put_creates_parents_and_leaves_no_partial() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let local = storage(&tmp);
        local
            .put("a/b/c.txt", Bytes::from_static(b"content"))
            .await
            .expect("put failed");
        assert_eq!(
            all_files(local.root()),
            vec!["a/b/c.txt".to_owned()],
            "put must leave exactly the final file, no partial"
        );
    }

    #[tokio::test]
    async fn get_missing_maps_not_found() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let local = storage(&tmp);
        let error = local.get("nope.txt").await.expect_err("get must fail");
        assert!(
            matches!(&error, StorageError::NotFound { key } if key == "nope.txt"),
            "expected NotFound, got: {error}"
        );
    }

    #[tokio::test]
    async fn list_filters_partial_files() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let local = storage(&tmp);
        local
            .put("wal/seg1", Bytes::from_static(b"1"))
            .await
            .expect("put failed");
        // Simulate a crashed mid-put: a stale partial file in the tree.
        std::fs::write(
            local.root().join("wal/seg2.partial-deadbeef"),
            b"incomplete",
        )
        .expect("write partial failed");
        let keys = local.list("").await.expect("list failed");
        assert_eq!(
            keys,
            vec!["wal/seg1".to_owned()],
            "list must filter out partial files"
        );
    }

    #[tokio::test]
    async fn list_walks_deep_trees_iteratively() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let local = storage(&tmp);
        let dirs = (0..64).map(|depth| format!("d{depth}")).collect::<Vec<_>>();
        let key = format!("{}/leaf.txt", dirs.join("/"));
        local
            .put(&key, Bytes::from_static(b"deep"))
            .await
            .expect("put failed");
        let keys = local.list("").await.expect("list failed");
        assert_eq!(keys, vec![key], "deep tree must be fully listed");
    }

    #[tokio::test]
    async fn list_mid_component_prefix_matches() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let local = storage(&tmp);
        local
            .put("generations/2026a/x", Bytes::from_static(b"1"))
            .await
            .expect("put failed");
        local
            .put("generations/2027b/y", Bytes::from_static(b"2"))
            .await
            .expect("put failed");
        // S3 semantics: a prefix may end in the middle of a component.
        let keys = local.list("generations/2026").await.expect("list failed");
        assert_eq!(
            keys,
            vec!["generations/2026a/x".to_owned()],
            "mid-component prefix must match like S3"
        );
    }

    #[tokio::test]
    async fn delete_prefix_empty_prefix_keeps_root_usable() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let local = storage(&tmp);
        local
            .put("a/1.txt", Bytes::from_static(b"1"))
            .await
            .expect("put failed");
        local
            .put("top.txt", Bytes::from_static(b"2"))
            .await
            .expect("put failed");
        local.delete_prefix("").await.expect("delete_prefix failed");
        assert_eq!(
            local.list("").await.expect("list failed"),
            Vec::<String>::new(),
            "delete_prefix of empty prefix must remove everything"
        );
        // The root itself survives: a subsequent put works.
        local
            .put("again.txt", Bytes::from_static(b"3"))
            .await
            .expect("put after root wipe failed");
    }

    #[tokio::test]
    async fn multipart_drop_leaves_only_partial() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let local = storage(&tmp);
        let mut upload = local
            .start_multipart("snap/db.zst")
            .await
            .expect("start failed");
        upload
            .write_part(Bytes::from_static(b"chunk"))
            .await
            .expect("write failed");
        drop(upload);
        let files = all_files(local.root());
        assert_eq!(files.len(), 1, "exactly one file expected: {files:?}");
        let only = files.first().expect("one file");
        assert!(
            only.starts_with("snap/db.zst.partial-"),
            "dropped upload must leave only the partial file, got {only}"
        );
    }

    #[tokio::test]
    async fn multipart_finish_renames_partial_away() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let local = storage(&tmp);
        let mut upload = local
            .start_multipart("snap/db.zst")
            .await
            .expect("start failed");
        upload
            .write_part(Bytes::from_static(b"part one "))
            .await
            .expect("write failed");
        upload
            .write_part(Bytes::from_static(b"part two"))
            .await
            .expect("write failed");
        upload.finish().await.expect("finish failed");
        assert_eq!(
            all_files(local.root()),
            vec!["snap/db.zst".to_owned()],
            "finish must leave exactly the final file"
        );
        let got = local.get("snap/db.zst").await.expect("get failed");
        assert_eq!(
            got.as_ref(),
            b"part one part two".as_slice(),
            "finished object must concatenate all parts"
        );
    }

    #[tokio::test]
    async fn multipart_abort_removes_partial() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let local = storage(&tmp);
        let mut upload = local
            .start_multipart("snap/db.zst")
            .await
            .expect("start failed");
        upload
            .write_part(Bytes::from_static(b"chunk"))
            .await
            .expect("write failed");
        upload.abort().await.expect("abort failed");
        assert_eq!(
            all_files(local.root()),
            Vec::<String>::new(),
            "abort must remove the partial file"
        );
    }

    #[tokio::test]
    async fn sweep_removes_crash_orphaned_partials() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let local = storage(&tmp);
        local
            .put(
                "generations/g1/wal/e0/seg.wal.zst",
                Bytes::from_static(b"x"),
            )
            .await
            .expect("put");
        // A crash-orphaned partial: written directly, no cleanup path ran.
        let orphan = local
            .root()
            .join("generations/g1/snapshot.db.zst.partial-deadbeef");
        std::fs::write(orphan.as_std_path(), b"torn").expect("plant orphan");
        let log = slog::Logger::root(slog::Discard, slog::o!());
        local.abort_incomplete_uploads(&log).await;
        assert!(
            !orphan.as_std_path().exists(),
            "the sweep removes the crash-orphaned partial"
        );
        assert_eq!(
            local
                .get("generations/g1/wal/e0/seg.wal.zst")
                .await
                .unwrap(),
            Bytes::from_static(b"x"),
            "real objects survive the sweep"
        );
    }

    #[tokio::test]
    async fn put_error_removes_partial_file() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let local = storage(&tmp);
        // Pre-create the final key as a directory so the rename fails AFTER
        // the partial file has already been written: this exercises the
        // created-then-removed cleanup path, not just an early write failure.
        std::fs::create_dir_all(local.root().join("k").as_std_path())
            .expect("plant directory at key path");
        let error = local
            .put("k", Bytes::from_static(b"data"))
            .await
            .expect_err("put onto a directory key must fail");
        assert!(
            matches!(error, StorageError::Local(_)),
            "the rename failure surfaces as a local error: {error}"
        );
        let partials: Vec<String> = all_files(local.root())
            .into_iter()
            .filter(|file| file.contains(PARTIAL_INFIX))
            .collect();
        assert!(
            partials.is_empty(),
            "a failed put must leave no partial file behind, found: {partials:?}"
        );
    }

    #[tokio::test]
    async fn object_methods_reject_escaping_keys() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let local = storage(&tmp);
        for key in ["../escape", "/leading", "gen/../escape", "gen//doubled"] {
            let error = local
                .put(key, Bytes::from_static(b"x"))
                .await
                .expect_err("put must reject an escaping key");
            assert!(
                matches!(&error, StorageError::InvalidKey { key: found, .. } if found == key),
                "put({key}) must be InvalidKey, got: {error}"
            );
            assert!(
                matches!(local.get(key).await, Err(StorageError::InvalidKey { .. })),
                "get({key}) must be InvalidKey"
            );
            assert!(
                matches!(
                    local.delete(key).await,
                    Err(StorageError::InvalidKey { .. })
                ),
                "delete({key}) must be InvalidKey"
            );
            assert!(
                matches!(
                    local.start_multipart(key).await,
                    Err(StorageError::InvalidKey { .. })
                ),
                "start_multipart({key}) must be InvalidKey"
            );
        }
        // A rejected put wrote nothing at all, not even a partial.
        assert_eq!(
            all_files(local.root()),
            Vec::<String>::new(),
            "rejected keys must never touch the filesystem"
        );
    }
}
