#![cfg(all(feature = "plus", feature = "testing"))]
//! Storage backend contract suite.
//!
//! Every [`ReplicaStorage`] backend must satisfy the same observable
//! semantics (documented in `src/storage.rs`). The suite is table-driven:
//! the [`for_each_contract_case`] macro is the single case table, and each
//! backend module instantiates one test per case against a fresh, isolated
//! backend. The suite runs here against (a) the local-filesystem backend and
//! (b) the fault-injection wrapper around it with an empty failure plan
//! (proving the wrapper is transparent). The live-S3 leg in
//! `tests/live_s3.rs` includes this file as a module and runs the same table
//! against a real bucket.
//!
//! NOTE: `unused_crate_dependencies` cannot be handled with a crate-level
//! `#![expect]` here because this file is also compiled as a module of
//! `tests/live_s3.rs`, where such an expectation is ignored and reported as
//! unfulfilled. Unused package dependencies are referenced explicitly
//! instead, as rustc recommends.

use async_compression as _;
use aws_credential_types as _;
use aws_sdk_s3 as _;
use bencher_json as _;
use hex as _;
use rand as _;
use rusqlite as _;
use serde as _;
use serde_json as _;
use sha2 as _;
use slog as _;
use thiserror as _;
use uuid as _;
use zstd as _;
// Optional dependency enabled by the otel feature; unused by tests.
#[cfg(feature = "otel")]
use bencher_otel as _;

/// The contract case table: expands `$per_case!(case_name);` once per case.
///
/// Each case is an `async fn(&ReplicaStorage)` in [`cases`]. Backend modules
/// pass a macro that wraps one case into one `#[tokio::test]`, so the case
/// name is part of the failing test's name and panic message.
macro_rules! for_each_contract_case {
    ($per_case:ident) => {
        $per_case!(put_then_get_roundtrip);
        $per_case!(get_missing_is_not_found_error);
        $per_case!(put_overwrite_replaces);
        $per_case!(list_prefix_returns_only_prefix);
        $per_case!(list_ordering_lexicographic);
        $per_case!(list_empty_prefix_ok_empty_vec);
        $per_case!(list_dirs_immediate_components_sorted);
        $per_case!(delete_then_get_not_found);
        $per_case!(delete_missing_idempotent_ok);
        $per_case!(delete_prefix_removes_all);
        $per_case!(multipart_roundtrip_large_object);
        $per_case!(multipart_abort_leaves_nothing);
        $per_case!(multipart_unfinished_invisible);
        $per_case!(key_charset_roundtrip);
        $per_case!(concurrent_puts_distinct_keys);
        $per_case!(get_stream_roundtrip);
    };
}

/// The generic contract cases. Each takes only a `&ReplicaStorage` rooted at
/// a fresh, empty, isolated namespace.
#[cfg(test)]
pub(crate) mod cases {
    use bencher_replica::{ReplicaStorage, StorageError};
    use bytes::Bytes;
    use pretty_assertions::assert_eq;
    use tokio::io::AsyncReadExt as _;

    /// One mebibyte, for sizing multipart writes.
    const MIB: usize = 1024 * 1024;

    /// Deterministic payload of `len` bytes (no RNG, no wall clock).
    fn pattern(len: usize) -> Vec<u8> {
        let unit: Vec<u8> = (0..=u8::MAX).collect();
        let mut data = unit.repeat(len.div_ceil(unit.len()));
        data.truncate(len);
        data
    }

    async fn put_str(storage: &ReplicaStorage, key: &str, value: &str) {
        storage
            .put(key, Bytes::copy_from_slice(value.as_bytes()))
            .await
            .unwrap_or_else(|error| panic!("put {key} failed: {error}"));
    }

    fn assert_not_found(result: Result<Bytes, StorageError>, key: &str, context: &str) {
        match result {
            Ok(bytes) => panic!(
                "{context}: get {key} returned Ok with {} bytes, expected NotFound",
                bytes.len()
            ),
            Err(StorageError::NotFound { key: found }) => {
                assert_eq!(found, key, "{context}: NotFound names the wrong key");
            },
            Err(error) => panic!("{context}: get {key} returned unexpected error: {error}"),
        }
    }

    pub(crate) async fn put_then_get_roundtrip(storage: &ReplicaStorage) {
        let key = "roundtrip/hello.txt";
        let value = Bytes::from_static(b"hello replica");
        storage.put(key, value.clone()).await.expect("put failed");
        let got = storage.get(key).await.expect("get failed");
        assert_eq!(got, value, "roundtrip value mismatch");
    }

    pub(crate) async fn get_missing_is_not_found_error(storage: &ReplicaStorage) {
        let result = storage.get("missing/never-written.txt").await;
        assert_not_found(
            result,
            "missing/never-written.txt",
            "get_missing_is_not_found_error",
        );
    }

    pub(crate) async fn put_overwrite_replaces(storage: &ReplicaStorage) {
        let key = "overwrite/value.txt";
        put_str(storage, key, "first").await;
        put_str(storage, key, "second, longer than the first").await;
        let got = storage.get(key).await.expect("get failed");
        assert_eq!(
            got.as_ref(),
            b"second, longer than the first".as_slice(),
            "overwrite did not replace the value"
        );
    }

    pub(crate) async fn list_prefix_returns_only_prefix(storage: &ReplicaStorage) {
        put_str(storage, "gen/a/1", "1").await;
        put_str(storage, "gen/a/2", "2").await;
        put_str(storage, "gen/b/1", "3").await;
        put_str(storage, "other/x", "4").await;
        let keys = storage.list("gen/a/").await.expect("list failed");
        assert_eq!(
            keys,
            vec!["gen/a/1".to_owned(), "gen/a/2".to_owned()],
            "list gen/a/ returned keys outside the prefix"
        );
        let keys = storage.list("gen/").await.expect("list failed");
        assert_eq!(
            keys,
            vec![
                "gen/a/1".to_owned(),
                "gen/a/2".to_owned(),
                "gen/b/1".to_owned()
            ],
            "list gen/ returned keys outside the prefix"
        );
    }

    pub(crate) async fn list_ordering_lexicographic(storage: &ReplicaStorage) {
        // Insert in scrambled order; list must sort lexicographically,
        // including "a" < "aa" < "b" and descent into nested keys.
        put_str(storage, "seg/b", "1").await;
        put_str(storage, "seg/aa", "2").await;
        put_str(storage, "seg/c/d", "3").await;
        put_str(storage, "seg/a", "4").await;
        let keys = storage.list("seg/").await.expect("list failed");
        assert_eq!(
            keys,
            vec![
                "seg/a".to_owned(),
                "seg/aa".to_owned(),
                "seg/b".to_owned(),
                "seg/c/d".to_owned()
            ],
            "list is not sorted lexicographically"
        );
    }

    pub(crate) async fn list_empty_prefix_ok_empty_vec(storage: &ReplicaStorage) {
        let keys = storage
            .list("")
            .await
            .expect("list of empty storage failed");
        assert_eq!(keys, Vec::<String>::new(), "empty storage must list empty");
        let keys = storage
            .list("no/such/prefix/")
            .await
            .expect("list of missing prefix failed");
        assert_eq!(keys, Vec::<String>::new(), "missing prefix must list empty");
    }

    pub(crate) async fn list_dirs_immediate_components_sorted(storage: &ReplicaStorage) {
        put_str(
            storage,
            "generations/20260710T145900Z-3f8a2c1d/snapshot.json",
            "a",
        )
        .await;
        put_str(
            storage,
            "generations/20260101T000000Z-0b1c2d3e/snapshot.json",
            "b",
        )
        .await;
        put_str(
            storage,
            "generations/20260315T120000Z-99aabbcc/wal/0000000000-9d2f1c4a8b3e6f70/x",
            "c",
        )
        .await;
        // A plain object at the listed level must not appear as a directory.
        put_str(storage, "rootfile", "d").await;

        let expected = vec![
            "20260101T000000Z-0b1c2d3e".to_owned(),
            "20260315T120000Z-99aabbcc".to_owned(),
            "20260710T145900Z-3f8a2c1d".to_owned(),
        ];
        let dirs = storage
            .list_dirs("generations/")
            .await
            .expect("list_dirs failed");
        assert_eq!(dirs, expected, "list_dirs returned wrong components");
        // A prefix without a trailing slash behaves the same.
        let dirs = storage
            .list_dirs("generations")
            .await
            .expect("list_dirs without trailing slash failed");
        assert_eq!(
            dirs, expected,
            "list_dirs must normalize the trailing slash"
        );

        let root_dirs = storage.list_dirs("").await.expect("list_dirs root failed");
        assert_eq!(
            root_dirs,
            vec!["generations".to_owned()],
            "root list_dirs must contain only directories, not objects"
        );
    }

    pub(crate) async fn delete_then_get_not_found(storage: &ReplicaStorage) {
        let key = "delete/me.txt";
        put_str(storage, key, "ephemeral").await;
        storage.delete(key).await.expect("delete failed");
        assert_not_found(storage.get(key).await, key, "delete_then_get_not_found");
    }

    pub(crate) async fn delete_missing_idempotent_ok(storage: &ReplicaStorage) {
        storage
            .delete("never/existed.txt")
            .await
            .expect("delete of missing key must be Ok");
        storage
            .delete("never/existed.txt")
            .await
            .expect("repeated delete of missing key must be Ok");
    }

    pub(crate) async fn delete_prefix_removes_all(storage: &ReplicaStorage) {
        put_str(storage, "gens/old/snapshot.db.zst", "s").await;
        put_str(storage, "gens/old/wal/0/seg1", "w1").await;
        put_str(storage, "gens/old/wal/1/seg2", "w2").await;
        put_str(storage, "gens/new/snapshot.db.zst", "keep").await;
        storage
            .delete_prefix("gens/old/")
            .await
            .expect("delete_prefix failed");
        let keys = storage
            .list("")
            .await
            .expect("list after delete_prefix failed");
        assert_eq!(
            keys,
            vec!["gens/new/snapshot.db.zst".to_owned()],
            "delete_prefix must remove exactly the prefixed keys"
        );
        storage
            .delete_prefix("gens/missing/")
            .await
            .expect("delete_prefix of missing prefix must be Ok");
    }

    pub(crate) async fn multipart_roundtrip_large_object(storage: &ReplicaStorage) {
        // ~12 MiB plus an unaligned tail: exercises the S3 backend's 5 MiB
        // minimum-part buffering across several parts.
        let data = pattern(12 * MIB + 513);
        let key = "snapshots/large/snapshot.db.zst";
        let mut upload = storage.start_multipart(key).await.expect("start failed");
        let part_sizes = [MIB, 6 * MIB, 4 * MIB, MIB, 513];
        assert_eq!(
            part_sizes.iter().sum::<usize>(),
            data.len(),
            "part sizes must cover the payload"
        );
        let mut remaining = Bytes::from(data.clone());
        for size in part_sizes {
            let part = remaining.split_to(size);
            upload.write_part(part).await.expect("write_part failed");
        }
        upload.finish().await.expect("finish failed");

        let got = storage.get(key).await.expect("get of large object failed");
        assert_eq!(got.len(), data.len(), "large object length mismatch");
        assert!(
            got.as_ref() == data.as_slice(),
            "large object content mismatch"
        );
        let keys = storage.list("snapshots/").await.expect("list failed");
        assert_eq!(keys, vec![key.to_owned()], "large object missing from list");
    }

    pub(crate) async fn multipart_abort_leaves_nothing(storage: &ReplicaStorage) {
        let key = "snapshots/aborted/snapshot.db.zst";
        let mut upload = storage.start_multipart(key).await.expect("start failed");
        upload
            .write_part(Bytes::from(pattern(MIB)))
            .await
            .expect("write_part failed");
        upload.abort().await.expect("abort failed");
        assert_not_found(
            storage.get(key).await,
            key,
            "multipart_abort_leaves_nothing",
        );
        let keys = storage.list("").await.expect("list after abort failed");
        assert_eq!(keys, Vec::<String>::new(), "abort must leave no objects");
    }

    pub(crate) async fn multipart_unfinished_invisible(storage: &ReplicaStorage) {
        let key = "snapshots/unfinished/snapshot.db.zst";
        let mut upload = storage.start_multipart(key).await.expect("start failed");
        upload
            .write_part(Bytes::from(pattern(MIB)))
            .await
            .expect("write_part failed");
        // Drop without finish: the object must never appear under its final
        // key (local: only a partial temp file; S3: uncompleted upload).
        drop(upload);
        assert_not_found(
            storage.get(key).await,
            key,
            "multipart_unfinished_invisible",
        );
        let keys = storage.list("").await.expect("list after drop failed");
        assert_eq!(
            keys,
            Vec::<String>::new(),
            "an unfinished multipart upload must not be listable"
        );
    }

    pub(crate) async fn key_charset_roundtrip(storage: &ReplicaStorage) {
        // A real replica key: generation id, epoch dir with salts, and a
        // zero-padded offset range segment name.
        let key = "generations/20260710T145900Z-3f8a2c1d/wal/0000000000-9d2f1c4a8b3e6f70/00000000000000000000-00000000000000524320.wal.zst";
        let value = Bytes::from(pattern(1024));
        storage.put(key, value.clone()).await.expect("put failed");
        let got = storage.get(key).await.expect("get failed");
        assert_eq!(got, value, "key charset roundtrip value mismatch");
        let keys = storage.list("generations/").await.expect("list failed");
        assert_eq!(
            keys,
            vec![key.to_owned()],
            "key charset key missing from list"
        );
    }

    pub(crate) async fn concurrent_puts_distinct_keys(storage: &ReplicaStorage) {
        let puts = (0..8)
            .map(|index| {
                let key = format!("concurrent/{index:02}.txt");
                let value = Bytes::from(format!("value {index}"));
                async move {
                    storage
                        .put(&key, value)
                        .await
                        .expect("concurrent put failed");
                    key
                }
            })
            .collect::<Vec<_>>();
        let mut expected = futures::future::join_all(puts).await;
        expected.sort();
        let keys = storage.list("concurrent/").await.expect("list failed");
        assert_eq!(keys, expected, "concurrent puts must all be visible");
        for (index, key) in keys.iter().enumerate() {
            let got = storage.get(key).await.expect("get failed");
            assert_eq!(
                got,
                Bytes::from(format!("value {index}")),
                "concurrent put content mismatch"
            );
        }
    }

    pub(crate) async fn get_stream_roundtrip(storage: &ReplicaStorage) {
        let data = pattern(3 * MIB + 7);
        let key = "stream/blob.bin";
        storage
            .put(key, Bytes::from(data.clone()))
            .await
            .expect("put failed");
        let mut stream = storage.get_stream(key).await.expect("get_stream failed");
        let mut got = Vec::new();
        stream
            .read_to_end(&mut got)
            .await
            .expect("reading stream failed");
        assert_eq!(got.len(), data.len(), "streamed length mismatch");
        assert!(got == data, "streamed content mismatch");

        match storage.get_stream("stream/missing.bin").await {
            Ok(_) => panic!("get_stream of a missing key must be NotFound"),
            Err(StorageError::NotFound { key: found }) => {
                assert_eq!(found, "stream/missing.bin", "NotFound names the wrong key");
            },
            Err(error) => panic!("get_stream returned unexpected error: {error}"),
        }
    }
}

/// Shared backend constructors for the suites in this file.
#[cfg(test)]
pub(crate) mod harness {
    use bencher_replica::testing::{FailurePlan, FlakyStorage};
    use bencher_replica::{LocalStorage, ReplicaStorage};
    use camino::Utf8Path;

    pub(crate) fn local_storage(tmp: &tempfile::TempDir) -> ReplicaStorage {
        let root = Utf8Path::from_path(tmp.path())
            .expect("tempdir path is UTF-8")
            .to_path_buf();
        ReplicaStorage::Local(LocalStorage::new(root))
    }

    pub(crate) fn flaky_storage(tmp: &tempfile::TempDir, plan: FailurePlan) -> ReplicaStorage {
        ReplicaStorage::Flaky(Box::new(FlakyStorage::new(local_storage(tmp), plan)))
    }
}

/// The contract suite against the local-filesystem backend in a tempdir.
#[cfg(test)]
mod local_backend {
    macro_rules! local_case {
        ($case:ident) => {
            #[tokio::test]
            async fn $case() {
                let tmp = tempfile::tempdir().expect("tempdir failed");
                let storage = super::harness::local_storage(&tmp);
                super::cases::$case(&storage).await;
            }
        };
    }
    for_each_contract_case!(local_case);
}

/// The contract suite against `Flaky(Local)` with an empty failure plan:
/// proves the fault-injection wrapper is transparent when no rule fires.
#[cfg(test)]
mod flaky_passthrough {
    macro_rules! flaky_case {
        ($case:ident) => {
            #[tokio::test]
            async fn $case() {
                let tmp = tempfile::tempdir().expect("tempdir failed");
                let storage = super::harness::flaky_storage(
                    &tmp,
                    bencher_replica::testing::FailurePlan::new(),
                );
                super::cases::$case(&storage).await;
            }
        };
    }
    for_each_contract_case!(flaky_case);
}

/// Flaky-only contract cases: these require a scripted failure plan, so they
/// cannot run against a plain backend.
#[cfg(test)]
mod flaky_only {
    use bencher_replica::testing::{FailurePlan, OpKind};
    use bencher_replica::{ReplicaStorage, StorageError};
    use bytes::Bytes;
    use pretty_assertions::assert_eq;

    /// The load-bearing case: a backend failure during `list` MUST surface
    /// as an `Err`, never as `Ok(vec![])`. Conflating the two would make an
    /// unreachable replica look empty and trigger a spurious new generation.
    #[tokio::test]
    async fn list_error_is_error_not_empty() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let storage =
            super::harness::flaky_storage(&tmp, FailurePlan::new().fail_all(OpKind::List));
        storage
            .put("generations/g1/snapshot.json", Bytes::from_static(b"{}"))
            .await
            .expect("put must pass through: only List is failed");

        match storage.list("").await {
            Ok(keys) => panic!(
                "list during an outage returned Ok({keys:?}): an unreachable \
                 replica must never look empty"
            ),
            Err(StorageError::Injected { op, .. }) => {
                assert_eq!(op, "list", "injected error names the wrong op");
            },
            Err(error) => panic!("list returned unexpected error: {error}"),
        }

        // After healing, the object put before the outage is visible again.
        let ReplicaStorage::Flaky(flaky) = &storage else {
            panic!("flaky_storage must build the Flaky variant");
        };
        flaky.heal();
        let keys = storage.list("").await.expect("list after heal failed");
        assert_eq!(
            keys,
            vec!["generations/g1/snapshot.json".to_owned()],
            "healed list must show the pre-outage object"
        );
    }
}

/// Local-only behaviors that diverge from S3 by design (documented in
/// `src/storage.rs`). Pinned here for the local backend so they cannot change
/// silently; the engine only ever uses directory-aligned prefixes and never
/// relies on the divergent cases.
#[cfg(test)]
mod local_divergences {
    use bencher_replica::StorageError;
    use bytes::Bytes;
    use pretty_assertions::assert_eq;

    /// Local `get` of a key that is actually a directory errors (`EISDIR`),
    /// where S3 (no directories) would return `NotFound`.
    #[tokio::test]
    async fn get_of_directory_key_errors() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let storage = super::harness::local_storage(&tmp);
        storage
            .put("dir/child", Bytes::from_static(b"x"))
            .await
            .expect("put failed");
        match storage.get("dir").await {
            Err(StorageError::Local(_)) => {},
            other => panic!("local get of a directory key must be a local error, got: {other:?}"),
        }
    }

    /// Local `list` errors when a prefix names a path whose component is a
    /// regular file, where S3 returns an empty listing.
    #[tokio::test]
    async fn list_prefix_through_a_file_errors() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let storage = super::harness::local_storage(&tmp);
        storage
            .put("a", Bytes::from_static(b"x"))
            .await
            .expect("put failed");
        match storage.list("a/").await {
            Err(StorageError::Local(_)) => {},
            other => {
                panic!("local list through a regular file must be a local error, got: {other:?}")
            },
        }
    }

    /// Local `delete_prefix` treats its argument as a directory path, so a
    /// prefix that ends mid-component matches no directory and no-ops, where
    /// S3 would delete every key with that raw string prefix.
    #[tokio::test]
    async fn delete_prefix_mid_component_noops() {
        let tmp = tempfile::tempdir().expect("tempdir failed");
        let storage = super::harness::local_storage(&tmp);
        storage
            .put("generations/g/x", Bytes::from_static(b"keep"))
            .await
            .expect("put failed");
        // "gen" is not a directory-aligned prefix of "generations/...".
        storage
            .delete_prefix("gen")
            .await
            .expect("mid-component delete_prefix must be Ok");
        assert_eq!(
            storage.list("").await.expect("list failed"),
            vec!["generations/g/x".to_owned()],
            "a mid-component prefix must delete nothing on the local backend"
        );
    }
}
