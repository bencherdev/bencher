#![cfg(all(feature = "plus", feature = "testing"))]
//! Live-S3 leg of the storage contract suite (`#[ignore]`-gated).
//!
//! Runs the SAME case table as `tests/storage_contract.rs` (included below
//! as a module) against a real S3-compatible bucket, following the
//! connectivity-probe pattern of `plus/bencher_oci_storage/tests/conformance.rs`:
//! when the environment is not configured, every test skips cleanly.
//!
//! # Running
//!
//! ```sh
//! export BENCHER_REPLICA_TEST_S3_BUCKET=my-test-bucket
//! export BENCHER_REPLICA_TEST_S3_ACCESS_KEY_ID=...
//! export BENCHER_REPLICA_TEST_S3_SECRET_ACCESS_KEY=...
//! # Optional (MinIO/R2 need an endpoint; region defaults to us-east-1):
//! export BENCHER_REPLICA_TEST_S3_ENDPOINT=http://localhost:9000
//! export BENCHER_REPLICA_TEST_S3_REGION=us-east-1
//! export BENCHER_REPLICA_TEST_S3_PREFIX=ci-scratch
//! cargo nextest run -p bencher_replica --features plus,testing \
//!     --run-ignored ignored-only -E 'binary(live_s3)'
//! ```
//!
//! CI runs this tier against dockerized `MinIO` and `RustFS` on every rust
//! change (the `replica_live_s3` job in `.github/workflows/test.yml`). To
//! run locally without AWS, start either server the same way, e.g.:
//!
//! ```sh
//! docker run -d -p 9000:9000 \
//!     -e MINIO_ROOT_USER=bencher-test \
//!     -e MINIO_ROOT_PASSWORD=bencher-test-secret \
//!     minio/minio server /data
//! aws --endpoint-url http://127.0.0.1:9000 s3api create-bucket \
//!     --bucket bencher-replica-test
//! ```
//!
//! Known server differences the suite tolerates: `RustFS` (1.0.0-beta.10)
//! returns an empty `ListMultipartUploads`, so the orphan-sweep case skips
//! there (`MinIO` and AWS exercise it).
//!
//! Every test isolates itself under a randomized per-run key prefix and
//! deletes that prefix in teardown (even when the case fails). A case that
//! drops an unfinished multipart upload leaves an invisible uncompleted
//! upload behind; use a bucket lifecycle rule to expire those.

// The contract suite is shared by including the file as a module; its test
// modules also compile (and run) in this binary, which keeps the shared
// cases exercised even when the live environment is absent.
// Optional dependency enabled by the otel feature; unused by tests.
#[cfg(feature = "otel")]
use bencher_otel as _;
#[macro_use]
#[path = "storage_contract.rs"]
mod storage_contract;

#[cfg(test)]
mod live_s3_backend {
    use std::pin::Pin;

    use bencher_replica::{ReplicaStorage, S3Storage};
    use uuid::Uuid;

    /// Environment configuration for the live-S3 leg.
    struct LiveS3Config {
        bucket: String,
        endpoint: Option<String>,
        region: Option<String>,
        access_key_id: String,
        secret_access_key: String,
        prefix: Option<String>,
    }

    impl LiveS3Config {
        /// Read `BENCHER_REPLICA_TEST_S3_*`; `None` when the required
        /// variables are unset (the test then skips cleanly).
        fn from_env() -> Option<Self> {
            let var = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
            Some(Self {
                bucket: var("BENCHER_REPLICA_TEST_S3_BUCKET")?,
                access_key_id: var("BENCHER_REPLICA_TEST_S3_ACCESS_KEY_ID")?,
                secret_access_key: var("BENCHER_REPLICA_TEST_S3_SECRET_ACCESS_KEY")?,
                endpoint: var("BENCHER_REPLICA_TEST_S3_ENDPOINT"),
                region: var("BENCHER_REPLICA_TEST_S3_REGION"),
                prefix: var("BENCHER_REPLICA_TEST_S3_PREFIX"),
            })
        }

        /// A storage handle scoped to `run_prefix` under the optional
        /// configured base prefix.
        fn storage(&self, run_prefix: &str) -> ReplicaStorage {
            self.s3_storage(run_prefix, None)
        }

        /// A storage handle scoped to `run_prefix`, optionally capping the
        /// `ListObjectsV2` page size so pagination is exercised over a small
        /// object count.
        fn s3_storage(&self, run_prefix: &str, max_keys: Option<i32>) -> ReplicaStorage {
            let prefix = match &self.prefix {
                Some(base) => format!("{base}/{run_prefix}"),
                None => run_prefix.to_owned(),
            };
            let mut s3 = S3Storage::new(
                self.bucket.clone(),
                Some(prefix),
                self.endpoint.clone(),
                self.region.clone(),
                self.access_key_id.clone(),
                &self.secret_access_key,
            );
            if let Some(max_keys) = max_keys {
                s3.set_max_keys(max_keys);
            }
            ReplicaStorage::S3(Box::new(s3))
        }
    }

    #[expect(
        clippy::print_stderr,
        reason = "report the clean skip when the live environment is absent"
    )]
    fn report_skip() {
        eprintln!("BENCHER_REPLICA_TEST_S3_* not set, skipping live S3 test");
    }

    #[expect(
        clippy::print_stderr,
        reason = "report the clean skip when the server cannot express the case"
    )]
    fn report_incomplete_uploads_unsupported() {
        eprintln!(
            "server does not report incomplete multipart uploads \
             (ListMultipartUploads returned none); skipping the sweep case"
        );
    }

    /// Run one contract case against a live bucket under a fresh randomized
    /// prefix, then delete the prefix in teardown, failure or not. The case
    /// runs in a spawned task so a panic still reaches teardown; the
    /// original panic message is printed by the panic hook and the test is
    /// failed afterwards.
    async fn run_live_case<CaseFn>(case: &str, case_fn: CaseFn)
    where
        CaseFn: for<'s> FnOnce(&'s ReplicaStorage) -> Pin<Box<dyn Future<Output = ()> + Send + 's>>
            + Send
            + 'static,
    {
        let Some(config) = LiveS3Config::from_env() else {
            report_skip();
            return;
        };
        let run_prefix = format!(
            "bencher-replica-contract/{case}-{}",
            Uuid::new_v4().simple()
        );
        let storage = config.storage(&run_prefix);
        let outcome = tokio::spawn(async move { case_fn(&storage).await }).await;
        // Teardown: remove everything this case wrote, even on failure.
        config
            .storage(&run_prefix)
            .delete_prefix("")
            .await
            .unwrap_or_else(|error| {
                panic!("failed to clean up live S3 prefix {run_prefix}: {error}")
            });
        if let Err(error) = outcome {
            panic!("live S3 contract case {case} failed: {error}");
        }
    }

    macro_rules! live_case {
        ($case:ident) => {
            #[tokio::test]
            #[ignore = "requires BENCHER_REPLICA_TEST_S3_* and a live bucket"]
            async fn $case() {
                run_live_case(stringify!($case), |storage| {
                    Box::pin(crate::storage_contract::cases::$case(storage))
                })
                .await;
            }
        };
    }
    for_each_contract_case!(live_case);

    /// Continuation-token pagination: with a tiny page size, both `list` and
    /// `list_dirs` must reassemble the complete, sorted result across several
    /// `ListObjectsV2` round-trips. Objects are spread over several epoch
    /// directories so `list_dirs` (delimiter-scoped) paginates too.
    #[tokio::test]
    #[ignore = "requires BENCHER_REPLICA_TEST_S3_* and a live bucket"]
    async fn pagination_spans_continuation_tokens() {
        use bytes::Bytes;
        use pretty_assertions::assert_eq;

        const EPOCHS: u32 = 6;
        const PER_EPOCH: u32 = 4;

        let Some(config) = LiveS3Config::from_env() else {
            report_skip();
            return;
        };
        let run_prefix = format!(
            "bencher-replica-contract/pagination-{}",
            Uuid::new_v4().simple()
        );
        // Page size 5 over 6 epoch dirs x 4 segments = 24 objects: `list`
        // spans ~5 pages and `list_dirs` (6 common prefixes) spans 2.
        let storage = config.s3_storage(&run_prefix, Some(5));
        let outcome = tokio::spawn(async move {
            let mut expected_keys = Vec::new();
            for epoch in 0..EPOCHS {
                for seq in 0..PER_EPOCH {
                    let key = format!("generations/g/wal/epoch{epoch:02}/seg{seq:02}.wal.zst");
                    storage
                        .put(&key, Bytes::from(format!("{epoch}-{seq}")))
                        .await
                        .expect("paginated put");
                    expected_keys.push(key);
                }
            }
            expected_keys.sort();

            let listed = storage
                .list("generations/g/wal/")
                .await
                .expect("paginated list");
            assert_eq!(
                listed, expected_keys,
                "paginated list must return every key, sorted, across pages"
            );

            let dirs = storage
                .list_dirs("generations/g/wal/")
                .await
                .expect("paginated list_dirs");
            let expected_dirs: Vec<String> = (0..EPOCHS)
                .map(|epoch| format!("epoch{epoch:02}"))
                .collect();
            assert_eq!(
                dirs, expected_dirs,
                "paginated list_dirs must return every directory, sorted, across pages"
            );
        })
        .await;
        // Teardown: remove everything this case wrote, even on failure.
        config
            .s3_storage(&run_prefix, None)
            .delete_prefix("")
            .await
            .unwrap_or_else(|error| {
                panic!("failed to clean up live S3 prefix {run_prefix}: {error}")
            });
        if let Err(error) = outcome {
            panic!("live S3 pagination case failed: {error}");
        }
    }

    /// Crash-orphan reconciliation: an incomplete multipart upload left behind
    /// by a killed process is reclaimed by the best-effort sweep. Start a
    /// multipart upload, drop it unfinished, sweep, and assert no incomplete
    /// upload remains under the prefix.
    #[tokio::test]
    #[ignore = "requires BENCHER_REPLICA_TEST_S3_* and a live bucket"]
    async fn abort_incomplete_uploads_sweeps_orphans() {
        use bencher_replica::ReplicaStorage;

        let Some(config) = LiveS3Config::from_env() else {
            report_skip();
            return;
        };
        let run_prefix = format!(
            "bencher-replica-contract/orphan-sweep-{}",
            Uuid::new_v4().simple()
        );
        let log = slog::Logger::root(slog::Discard, slog::o!());
        let storage = config.s3_storage(&run_prefix, None);
        let outcome = tokio::spawn(async move {
            // Start a multipart upload and drop it unfinished: a crash-orphaned
            // incomplete upload now exists server-side under the prefix.
            let upload = storage
                .start_multipart("snap/db.zst")
                .await
                .expect("start multipart");
            drop(upload);
            let ReplicaStorage::S3(s3) = &storage else {
                panic!("expected the S3 backend");
            };
            // Some S3-compatible servers (rustfs 1.0.0-beta.10) return an
            // empty ListMultipartUploads even when an incomplete upload
            // exists, so the sweep has nothing observable to reclaim there:
            // skip rather than fail, and rely on the AWS S3/MinIO legs to
            // exercise the sweep.
            if s3.incomplete_upload_count().await == 0 {
                report_incomplete_uploads_unsupported();
                return;
            }
            // Sweep through the enum method: callers need no backend match.
            storage.abort_incomplete_uploads(&log).await;
            assert_eq!(
                s3.incomplete_upload_count().await,
                0,
                "the sweep aborts every incomplete upload under the prefix"
            );
        })
        .await;
        // Teardown: abort any straggler upload and remove written objects.
        let cleanup_log = slog::Logger::root(slog::Discard, slog::o!());
        config
            .s3_storage(&run_prefix, None)
            .abort_incomplete_uploads(&cleanup_log)
            .await;
        config
            .s3_storage(&run_prefix, None)
            .delete_prefix("")
            .await
            .unwrap_or_else(|error| {
                panic!("failed to clean up live S3 prefix {run_prefix}: {error}")
            });
        if let Err(error) = outcome {
            panic!("live S3 orphan-sweep case failed: {error}");
        }
    }
}
