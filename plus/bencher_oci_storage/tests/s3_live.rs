//! Live S3 registry storage test
//!
//! Exercises the full registry flow against a real S3-compatible bucket.
//! Ignored by default and never run in CI: it needs credentials for a scratch
//! bucket, supplied through the environment.
//!
//! ```sh
//! export BENCHER_REGISTRY_TEST_S3_BUCKET=my-scratch-bucket
//! export BENCHER_REGISTRY_TEST_S3_ENDPOINT=https://s3.example.com
//! export BENCHER_REGISTRY_TEST_S3_REGION=auto
//! export BENCHER_REGISTRY_TEST_S3_ACCESS_KEY_ID=...
//! export BENCHER_REGISTRY_TEST_S3_SECRET_ACCESS_KEY=...
//! export BENCHER_REGISTRY_TEST_S3_PATH=registry-test
//! cargo test -p bencher_oci_storage --features plus,test-clock --test s3_live -- --ignored --nocapture
//! ```
//!
//! Every run works under its own key prefix, so concurrent runs and reruns do
//! not collide and leftovers stay out of the way.

#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::missing_assert_message,
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "integration test file"
)]

use bencher_json::system::config::RegistryStorage;
use bencher_oci_storage::{Digest, OciStorage, OciStorageError, Tag};
use bytes::Bytes;

const BUCKET: &str = "BENCHER_REGISTRY_TEST_S3_BUCKET";
const ENDPOINT: &str = "BENCHER_REGISTRY_TEST_S3_ENDPOINT";
const REGION: &str = "BENCHER_REGISTRY_TEST_S3_REGION";
const ACCESS_KEY_ID: &str = "BENCHER_REGISTRY_TEST_S3_ACCESS_KEY_ID";
const SECRET_ACCESS_KEY: &str = "BENCHER_REGISTRY_TEST_S3_SECRET_ACCESS_KEY";
const PATH: &str = "BENCHER_REGISTRY_TEST_S3_PATH";

const PROJECT: &str = "00000000-0000-0000-0000-0000000000a1";
const OTHER_PROJECT: &str = "00000000-0000-0000-0000-0000000000a2";
const JOB: &str = "00000000-0000-0000-0000-0000000000b1";

/// The key prefix for this run, or `None` when the variables are absent.
///
/// Every run works under its own prefix, so concurrent runs and reruns never
/// collide.
fn run_path() -> Option<String> {
    if std::env::var(BUCKET).is_err() {
        eprintln!("{BUCKET} is not set, skipping the live S3 registry test");
        return None;
    }
    if std::env::var(ACCESS_KEY_ID).is_err() || std::env::var(SECRET_ACCESS_KEY).is_err() {
        eprintln!(
            "{ACCESS_KEY_ID} and {SECRET_ACCESS_KEY} must both be set, skipping the live S3 registry test"
        );
        return None;
    }

    let run = uuid::Uuid::new_v4();
    let path = match std::env::var(PATH) {
        Ok(path) if !path.is_empty() => format!("{path}/{run}"),
        _ => run.to_string(),
    };
    println!("Live S3 registry test running under prefix: {path}");
    Some(path)
}

/// Builds storage over the run's key prefix with the given upload timeout.
fn build_storage(path: &str, upload_timeout: u64) -> OciStorage {
    let log = slog::Logger::root(slog::Discard, slog::o!());
    OciStorage::try_from_config(
        log,
        Some(RegistryStorage::S3 {
            bucket: std::env::var(BUCKET).expect("Bucket is set"),
            path: Some(path.to_owned()),
            endpoint: std::env::var(ENDPOINT).ok().filter(|e| !e.is_empty()),
            region: std::env::var(REGION).ok().filter(|r| !r.is_empty()),
            access_key_id: std::env::var(ACCESS_KEY_ID).expect("Access key is set"),
            secret_access_key: std::env::var(SECRET_ACCESS_KEY)
                .expect("Secret is set")
                .parse()
                .expect("Invalid secret"),
            chunk_size: None,
        }),
        std::path::Path::new("bencher.db"),
        Some(upload_timeout),
        None,
        None,
    )
    .expect("Failed to build S3 registry storage")
}

fn manifest_json(tag: &str) -> String {
    serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "config": {
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "digest": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "size": 0
        },
        "layers": [],
        "annotations": { "tag": tag }
    })
    .to_string()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a scratch S3 bucket and credentials"]
async fn s3_registry_flow() {
    let Some(path) = run_path() else {
        return;
    };
    let storage = build_storage(&path, 3600);
    let project = PROJECT.parse().expect("Invalid project UUID");
    let other = OTHER_PROJECT.parse().expect("Invalid project UUID");

    blob_upload(&storage, &project).await;
    digest_mismatch(&storage, &project).await;
    let blob = blob_operations(&storage, &project, &other).await;
    manifests_and_tags(&storage, &project).await;
    stale_upload_cleanup(&storage, &path, &project).await;
    job_output(&storage, &project).await;

    // The run's own uuid prefix isolates everything else
    storage
        .delete_blob(&project, &blob)
        .await
        .expect("Failed to delete blob");
    storage
        .delete_blob(&other, &blob)
        .await
        .expect("Failed to delete mounted blob");
}

/// Several appends, one under and one over `chunk_size`, then a correct digest.
async fn blob_upload(storage: &OciStorage, project: &bencher_oci_storage::ProjectUuid) {
    let chunk_size = storage.chunk_size();
    let small = Bytes::from_static(b"a small first chunk");
    let large = Bytes::from(vec![b'x'; chunk_size + 1024]);
    let tail = Bytes::from_static(b"and a small tail");

    let upload_id = storage
        .start_upload(project)
        .await
        .expect("Failed to start upload");
    storage
        .validate_upload_repository(&upload_id, project)
        .await
        .expect("Upload belongs to another project");

    let mut expected = Vec::new();
    for chunk in [&small, &large, &tail] {
        expected.extend_from_slice(chunk);
        let size = storage
            .append_upload(&upload_id, chunk.clone())
            .await
            .expect("Failed to append upload");
        assert_eq!(size, expected.len() as u64, "cumulative size");
    }
    assert_eq!(
        storage
            .get_upload_size(&upload_id)
            .await
            .expect("Failed to get upload size"),
        expected.len() as u64
    );

    let digest = Digest::from_sha256_bytes(&expected);
    let stored = storage
        .complete_upload(&upload_id, &digest)
        .await
        .expect("Failed to complete upload");
    assert_eq!(stored.as_str(), digest.as_str());

    // The blob round trips, whole and streamed
    let (data, size) = storage
        .get_blob(project, &stored)
        .await
        .expect("Failed to get blob");
    assert_eq!(data.as_ref(), expected.as_slice());
    assert_eq!(size, expected.len() as u64);

    let (body, size) = storage
        .get_blob_stream(project, &stored)
        .await
        .expect("Failed to stream blob");
    assert_eq!(size, expected.len() as u64);
    let streamed = http_body_util::BodyExt::collect(body)
        .await
        .expect("Failed to collect blob stream")
        .to_bytes();
    assert_eq!(streamed.as_ref(), expected.as_slice());

    storage
        .delete_blob(project, &stored)
        .await
        .expect("Failed to delete blob");
    assert!(
        !storage
            .blob_exists(project, &stored)
            .await
            .expect("Failed to check blob")
    );
}

/// A wrong digest fails and leaves no blob behind.
async fn digest_mismatch(storage: &OciStorage, project: &bencher_oci_storage::ProjectUuid) {
    let data = Bytes::from_static(b"content that will not match");
    let upload_id = storage
        .start_upload(project)
        .await
        .expect("Failed to start upload");
    storage
        .append_upload(&upload_id, data.clone())
        .await
        .expect("Failed to append upload");

    let wrong = Digest::from_sha256_bytes(b"a completely different payload");
    let error = storage
        .complete_upload(&upload_id, &wrong)
        .await
        .expect_err("A wrong digest must fail");
    assert!(
        matches!(error, OciStorageError::DigestMismatch { .. }),
        "{error}"
    );

    assert!(
        !storage
            .blob_exists(project, &wrong)
            .await
            .expect("Failed to check blob"),
        "no blob at the claimed digest"
    );
    let actual = Digest::from_sha256_bytes(&data);
    assert!(
        !storage
            .blob_exists(project, &actual)
            .await
            .expect("Failed to check blob"),
        "no blob at the actual digest"
    );
    // The failed session is cleaned up
    storage
        .get_upload_size(&upload_id)
        .await
        .expect_err("The failed session must be gone");
}

/// Blob existence, size, and a cross-project mount.
async fn blob_operations(
    storage: &OciStorage,
    project: &bencher_oci_storage::ProjectUuid,
    other: &bencher_oci_storage::ProjectUuid,
) -> Digest {
    let data = Bytes::from_static(b"a mountable blob");
    let upload_id = storage
        .start_upload(project)
        .await
        .expect("Failed to start upload");
    storage
        .append_upload(&upload_id, data.clone())
        .await
        .expect("Failed to append upload");
    let stored = storage
        .complete_upload(&upload_id, &Digest::from_sha256_bytes(&data))
        .await
        .expect("Failed to complete upload");

    assert!(
        storage
            .blob_exists(project, &stored)
            .await
            .expect("Failed to check blob")
    );
    assert_eq!(
        storage
            .get_blob_size(project, &stored)
            .await
            .expect("Failed to get blob size"),
        data.len() as u64
    );

    assert!(
        storage
            .mount_blob(project, other, &stored)
            .await
            .expect("Failed to mount blob"),
        "the mount succeeds"
    );
    assert!(
        storage
            .blob_exists(other, &stored)
            .await
            .expect("Failed to check mounted blob")
    );

    let missing = Digest::from_sha256_bytes(b"never uploaded");
    assert!(
        !storage
            .mount_blob(project, other, &missing)
            .await
            .expect("Failed to mount blob"),
        "mounting a missing blob reports no mount"
    );

    stored
}

/// Manifest put and get by digest, tag resolve, and tag listing across pages.
async fn manifests_and_tags(storage: &OciStorage, project: &bencher_oci_storage::ProjectUuid) {
    let names = ["alpha", "beta", "delta", "epsilon", "gamma"];
    let mut digests = Vec::new();
    for name in names {
        let content = manifest_json(name);
        let manifest =
            bencher_json::oci::Manifest::from_bytes(content.as_bytes()).expect("Invalid manifest");
        let tag: Tag = name.parse().expect("Invalid tag");
        let stored = storage
            .put_manifest(project, Bytes::from(content.clone()), Some(&tag), &manifest)
            .await
            .expect("Failed to put manifest");

        assert!(
            storage
                .manifest_exists(project, &stored)
                .await
                .expect("Failed to check manifest")
        );
        assert_eq!(
            storage
                .get_manifest_by_digest(project, &stored)
                .await
                .expect("Failed to get manifest")
                .as_ref(),
            content.as_bytes()
        );
        assert_eq!(
            storage
                .resolve_tag(project, &tag)
                .await
                .expect("Failed to resolve tag")
                .as_str(),
            stored.as_str()
        );
        digests.push(stored);
    }

    let all = storage
        .list_tags(project, None, None)
        .await
        .expect("Failed to list tags");
    assert_eq!(all.tags, names, "tags are lexicographic");
    assert!(!all.has_more);

    // Walk more than one page
    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;
    let mut pages = 0;
    loop {
        let result = storage
            .list_tags(project, Some(2), cursor.as_deref())
            .await
            .expect("Failed to list tags");
        pages += 1;
        seen.extend(result.tags.iter().cloned());
        if !result.has_more {
            break;
        }
        cursor = result.tags.last().cloned();
    }
    assert!(pages > 1, "the listing spans more than one page");
    assert_eq!(seen, names);

    // Deleting a manifest takes its tag with it
    for digest in digests {
        storage
            .delete_manifest(project, &digest)
            .await
            .expect("Failed to delete manifest");
    }
    assert!(
        storage
            .list_tags(project, None, None)
            .await
            .expect("Failed to list tags")
            .tags
            .is_empty()
    );
}

/// A session older than the timeout is swept away by the background cleanup.
async fn stale_upload_cleanup(
    storage: &OciStorage,
    path: &str,
    project: &bencher_oci_storage::ProjectUuid,
) {
    let upload_id = storage
        .start_upload(project)
        .await
        .expect("Failed to start upload");
    storage
        .append_upload(&upload_id, Bytes::from_static(b"soon to be stale"))
        .await
        .expect("Failed to append upload");

    // Let the session age past a zero-second timeout
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // A fresh instance over the same prefix with a zero timeout treats every
    // existing session as stale, and its first `start_upload` sweeps them.
    let sweeper = build_storage(path, 0);
    let sweeper_upload = sweeper
        .start_upload(project)
        .await
        .expect("Failed to start upload");

    // The sweep runs in the background, so poll for it
    let mut swept = false;
    for _ in 0..30 {
        if storage.get_upload_size(&upload_id).await.is_err() {
            swept = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    assert!(swept, "the stale session must be swept away");

    sweeper
        .cancel_upload(&sweeper_upload)
        .await
        .expect("Failed to cancel upload");
}

/// Job output round trips and is scoped to the project.
async fn job_output(storage: &OciStorage, project: &bencher_oci_storage::ProjectUuid) {
    let job = JOB.parse().expect("Invalid job UUID");
    let output = bencher_json::runner::JsonJobOutput {
        results: vec![bencher_json::runner::JsonIterationOutput {
            exit_code: 0,
            stdout: Some("live stdout".into()),
            stderr: None,
            output: None,
        }],
        error: None,
    };

    storage
        .job_output()
        .put(*project, job, &output)
        .await
        .expect("Failed to put job output");
    let stored = storage
        .job_output()
        .get(*project, job)
        .await
        .expect("Failed to get job output")
        .expect("Job output is missing");
    assert_eq!(stored.results[0].stdout.as_deref(), Some("live stdout"));
}
