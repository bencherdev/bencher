//! Blobs by digest, apart from the image layout whose manifests name them.

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _};
use std::path::Path;

use camino::{Utf8Path, Utf8PathBuf};

use crate::error::OciError;

/// The directory an OCI image layout keeps its blobs in.
const BLOBS: &str = "blobs";

/// Blobs at `<dir>/<algorithm>/<hex>`, laid out as an OCI image layout's
/// `blobs` directory is.
#[derive(Debug, Clone)]
pub struct BlobStore {
    dir: Utf8PathBuf,
    /// For a store kept across pulls, the user whose private directories alone
    /// hold it, since its blobs are trusted when read.
    owner: Option<u32>,
}

impl BlobStore {
    #[must_use]
    pub fn new(dir: Utf8PathBuf) -> Self {
        Self { dir, owner: None }
    }

    /// A store whose every directory level, `dir` and each algorithm directory
    /// in it, must be a real directory `owner` owns and nobody else may write,
    /// or nothing below it is read, written, or removed.
    #[must_use]
    pub fn private(dir: Utf8PathBuf, owner: u32) -> Self {
        Self {
            dir,
            owner: Some(owner),
        }
    }

    /// The blobs of the image layout at `image_dir`.
    #[must_use]
    pub fn of_layout(image_dir: &Utf8Path) -> Self {
        Self::new(image_dir.join(BLOBS))
    }

    #[must_use]
    pub fn dir(&self) -> &Utf8Path {
        &self.dir
    }

    /// Where the blob of `digest` lives.
    ///
    /// The algorithm must be alphanumeric (plus `+`, `-`, `.`, `_`) per the OCI
    /// spec and the hex only hexadecimal, so no digest reaches outside the store.
    pub fn blob_path(&self, digest: &str) -> Result<Utf8PathBuf, OciError> {
        let (algorithm, hex) = digest
            .split_once(':')
            .ok_or_else(|| OciError::PathTraversal(format!("Invalid digest format: {digest}")))?;

        // OCI digest algorithm: [a-z][a-z0-9]*([+.-_][a-z][a-z0-9]*)*
        // We accept alphanumeric plus `+`, `-`, `.`, `_` which covers all valid algorithms.
        if algorithm.is_empty()
            || !algorithm.bytes().all(|b| {
                b.is_ascii_alphanumeric() || b == b'+' || b == b'-' || b == b'.' || b == b'_'
            })
        {
            return Err(OciError::PathTraversal(format!(
                "Invalid digest algorithm: {algorithm}"
            )));
        }

        if !is_hex(hex) {
            return Err(OciError::PathTraversal(format!(
                "Invalid digest hex: {hex}"
            )));
        }

        Ok(self.dir.join(algorithm).join(hex))
    }

    /// Where a pull keeps the blob of `digest`, its algorithm directory made
    /// private when missing.
    pub(crate) fn writable_path(&self, digest: &str) -> Result<Utf8PathBuf, OciError> {
        let path = self.blob_path(digest)?;
        let algorithm = path
            .parent()
            .ok_or_else(|| OciError::InvalidLayout(format!("No directory for blob {digest}")))?;
        self.check(self.dir.as_std_path())?;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(algorithm)?;
        self.check(algorithm.as_std_path())?;
        Ok(path)
    }

    /// Refuses a store that is not private to its owner at every level, every
    /// entry of it being an algorithm directory.
    pub fn verify(&self) -> Result<(), OciError> {
        self.check(self.dir.as_std_path())?;
        for entry in fs::read_dir(&self.dir)? {
            self.check(&entry?.path())?;
        }
        Ok(())
    }

    fn check(&self, dir: &Path) -> Result<(), OciError> {
        if let Some(owner) = self.owner
            && !private_to(dir, owner)?
        {
            return Err(OciError::ForeignBlobStore(dir.to_owned()));
        }
        Ok(())
    }

    /// Removes every file that is not a blob, which only a write cut off before
    /// its rename leaves, then the least recently stamped blobs until the rest
    /// total `cap` bytes or less. An entry that cannot be read or removed is
    /// counted, and costs only itself.
    ///
    /// Run only while nothing writes to the store.
    pub fn evict(&self, cap: u64) -> Result<Eviction, OciError> {
        let mut eviction = Eviction::default();
        let mut blobs = Vec::new();
        for entry in self.entries(&mut eviction)? {
            let path = entry.path();
            if entry.file_name().to_str().is_some_and(is_hex) {
                match entry
                    .metadata()
                    .and_then(|metadata| Ok((metadata.modified()?, metadata.len())))
                {
                    Ok((stamp, len)) => blobs.push((stamp, len, path)),
                    Err(_) => eviction.failed += 1,
                }
            } else if removed(&path, &mut eviction) {
                eviction.temporary += 1;
            }
        }

        blobs.sort_by_key(|(stamp, _, _)| *stamp);
        let mut total: u64 = blobs.iter().map(|(_, len, _)| len).sum();
        for (_, len, path) in blobs {
            if total <= cap {
                break;
            }
            if removed(&path, &mut eviction) {
                total -= len;
                eviction.blobs += 1;
                eviction.bytes += len;
            }
        }
        eviction.total = total;
        Ok(eviction)
    }

    /// Every entry of every algorithm directory, listed through `std` so a name
    /// that is not UTF-8 is listed too.
    fn entries(&self, eviction: &mut Eviction) -> Result<Vec<fs::DirEntry>, OciError> {
        // A store that is not there holds nothing.
        if self
            .dir
            .symlink_metadata()
            .is_err_and(|e| e.kind() == ErrorKind::NotFound)
        {
            return Ok(Vec::new());
        }
        self.check(self.dir.as_std_path())?;
        let mut entries = Vec::new();
        for algorithm in fs::read_dir(&self.dir)? {
            let Ok(algorithm) = algorithm else {
                eviction.failed += 1;
                continue;
            };
            // Never followed: a link, like a file, is no algorithm directory.
            if !algorithm.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let dir = algorithm.path();
            self.check(&dir)?;
            let Ok(listing) = fs::read_dir(&dir) else {
                eviction.failed += 1;
                continue;
            };
            for entry in listing {
                if let Ok(entry) = entry {
                    entries.push(entry);
                } else {
                    eviction.failed += 1;
                }
            }
        }
        Ok(entries)
    }
}

/// What an eviction pass removed, and what the store held after it.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Eviction {
    /// Temporary files left by writes that never finished.
    pub temporary: usize,
    pub blobs: usize,
    /// The bytes of the blobs removed.
    pub bytes: u64,
    /// The bytes of the blobs left.
    pub total: u64,
    /// Entries that could not be read or removed, each left for the next pass.
    pub failed: usize,
}

/// Only `owner`, or root, can have written into a real directory `owner` owns
/// and nobody else may write to.
pub fn private_to(dir: &Path, owner: u32) -> std::io::Result<bool> {
    let metadata = fs::symlink_metadata(dir)?;
    Ok(metadata.is_dir() && metadata.uid() == owner && metadata.mode() & 0o022 == 0)
}

fn is_hex(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A file already gone counts as removed.
fn removed(path: &Path, eviction: &mut Eviction) -> bool {
    if let Err(e) = fs::remove_file(path)
        && e.kind() != ErrorKind::NotFound
    {
        eviction.failed += 1;
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    use std::time::{Duration, SystemTime};

    use super::*;

    #[test]
    fn eviction_removes_the_oldest_stamps_first_down_to_the_cap() {
        // Kills evicting the newest first, or the largest, or everything once
        // over the cap, and stopping only once strictly under it.
        let (_dir, store) = store();
        let newest = put(&store, "aa", 8, 3);
        let oldest = put(&store, "bb", 4, 1);
        let middle = put(&store, "cc", 16, 2);

        let eviction = store.evict(24).unwrap();

        assert_eq!(
            eviction,
            Eviction {
                temporary: 0,
                blobs: 1,
                bytes: 4,
                total: 24,
                failed: 0,
            }
        );
        assert!(!oldest.exists());
        assert!(newest.exists() && middle.exists());
    }

    #[test]
    fn every_pass_removes_temporary_files_and_a_zero_cap_empties_the_store() {
        // Kills a pass that leaves a cut-off write behind, and a cap of zero
        // that keeps the store as it is.
        let (_dir, store) = store();
        let blob = put(&store, "aa", 8, 1);
        let temporary = store.dir().join("sha256").join(".tmpXyZ123");
        fs::write(&temporary, b"half").unwrap();

        let roomy = store.evict(u64::MAX).unwrap();
        assert_eq!(
            roomy,
            Eviction {
                temporary: 1,
                blobs: 0,
                bytes: 0,
                total: 8,
                failed: 0,
            }
        );
        assert!(!temporary.exists() && blob.exists());

        let emptied = store.evict(0).unwrap();
        assert_eq!(
            emptied,
            Eviction {
                temporary: 0,
                blobs: 1,
                bytes: 8,
                total: 0,
                failed: 0,
            }
        );
        assert!(!blob.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn an_entry_the_pass_cannot_list_or_remove_costs_only_itself() {
        // Kills a pass that stops at its first bad entry, a name that is not
        // UTF-8 or one it cannot remove, which then never evicts again and lets
        // the store outgrow its cap.
        use std::os::unix::ffi::OsStrExt as _;

        let (_dir, store) = store();
        let blob = put(&store, "aa", 8, 1);
        let sha256 = store.dir().join("sha256");
        let not_utf8 = sha256
            .as_std_path()
            .join(std::ffi::OsStr::from_bytes(b"\xff.tmp"));
        fs::write(&not_utf8, b"half").unwrap();
        let stuck = sha256.join(".stuck");
        fs::create_dir(&stuck).unwrap();

        let eviction = store.evict(0).unwrap();

        assert_eq!(
            eviction,
            Eviction {
                temporary: 1,
                blobs: 1,
                bytes: 8,
                total: 0,
                failed: 1,
            }
        );
        assert!(!not_utf8.exists() && !blob.exists() && stuck.exists());
    }

    #[test]
    fn a_pass_goes_below_no_level_that_is_not_the_stores_own() {
        // Kills a pass that follows a link where an algorithm directory belongs,
        // or descends into one somebody else may write, either of which deletes
        // files the store never held.
        let (linked_dir, linked) = private_store();
        let victim = Utf8Path::from_path(linked_dir.path())
            .unwrap()
            .join("victim");
        // Private, so only the link itself, never its target, can refuse it.
        fs::DirBuilder::new().mode(0o700).create(&victim).unwrap();
        symlink(&victim, linked.dir().join("sha256")).unwrap();
        let (_open_dir, open) = private_store();
        let writable = open.dir().join("sha256");
        fs::create_dir(&writable).unwrap();
        fs::set_permissions(&writable, fs::Permissions::from_mode(0o777)).unwrap();
        let (_wide_dir, wide) = private_store();
        let in_wide = wide.dir().join("sha256");
        fs::create_dir(&in_wide).unwrap();
        fs::set_permissions(wide.dir(), fs::Permissions::from_mode(0o777)).unwrap();
        let files: Vec<_> = [&victim, &writable, &in_wide]
            .into_iter()
            .flat_map(|dir| [dir.join("aa"), dir.join("some_config")])
            .collect();
        for file in &files {
            fs::write(file, b"theirs").unwrap();
        }

        let skipped = linked.evict(0).unwrap();
        let refused = [
            (open.evict(0), writable.as_path()),
            (wide.evict(0), wide.dir()),
        ];

        assert_eq!(skipped, Eviction::default());
        for (result, level) in &refused {
            assert!(
                matches!(result, Err(OciError::ForeignBlobStore(path)) if path.as_path() == level.as_std_path()),
                "{level}: {result:?}"
            );
        }
        for file in &files {
            assert!(file.exists(), "{file} was deleted");
        }
    }

    /// A store private to the test's user, as the runner keeps one.
    fn private_store() -> (tempfile::TempDir, BlobStore) {
        let dir = tempfile::tempdir().unwrap();
        let blobs = Utf8Path::from_path(dir.path()).unwrap().join("blobs");
        fs::DirBuilder::new().mode(0o700).create(&blobs).unwrap();
        let owner = fs::metadata(&blobs).unwrap().uid();
        (dir, BlobStore::private(blobs, owner))
    }

    fn store() -> (tempfile::TempDir, BlobStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::new(Utf8Path::from_path(dir.path()).unwrap().join("blobs"));
        (dir, store)
    }

    /// A blob of `len` bytes, stamped `stamp` seconds after the epoch.
    fn put(store: &BlobStore, hex: &str, len: usize, stamp: u64) -> Utf8PathBuf {
        let path = store.blob_path(&format!("sha256:{hex}")).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, vec![0; len]).unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(stamp))
            .unwrap();
        path
    }
}
