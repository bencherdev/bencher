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
use object_store::{ObjectStoreExt as _, path::Path};

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
    job_output(&storage, &path, &project).await;

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
    assert_eq!(
        storage
            .list_tags(project, None, None)
            .await
            .expect("Failed to list tags")
            .tags,
        Vec::<String>::new()
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
    // existing session as stale, and sweeps them once it is built.
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

    // The zero timeout sweep can take the sweeper's own session first
    let cancelled = sweeper.cancel_upload(&sweeper_upload).await;
    assert!(
        matches!(cancelled, Ok(()) | Err(OciStorageError::UploadNotFound(_))),
        "Failed to cancel upload: {cancelled:?}"
    );
}

/// Job output round trips and is scoped to the project.
async fn job_output(storage: &OciStorage, path: &str, project: &bencher_oci_storage::ProjectUuid) {
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

    // Storage has no job output delete, so remove the object directly
    let key = Path::from(path)
        .join(PROJECT)
        .join("output")
        .join("v0")
        .join("jobs")
        .join(JOB);
    raw_store()
        .delete(&key)
        .await
        .expect("Failed to delete job output");
    assert!(
        storage
            .job_output()
            .get(*project, job)
            .await
            .expect("Failed to get job output")
            .is_none(),
        "the job output is gone"
    );
}

/// A plain client for the scratch bucket, for cleanup the storage API does not offer.
fn raw_store() -> object_store::aws::AmazonS3 {
    let mut builder = object_store::aws::AmazonS3Builder::new()
        .with_bucket_name(std::env::var(BUCKET).expect("Bucket is set"))
        .with_access_key_id(std::env::var(ACCESS_KEY_ID).expect("Access key is set"))
        .with_secret_access_key(std::env::var(SECRET_ACCESS_KEY).expect("Secret is set"))
        .with_region(
            std::env::var(REGION)
                .ok()
                .filter(|r| !r.is_empty())
                .unwrap_or_else(|| "auto".to_owned()),
        );
    if let Some(endpoint) = std::env::var(ENDPOINT).ok().filter(|e| !e.is_empty()) {
        builder = builder
            .with_allow_http(endpoint.starts_with("http://"))
            .with_endpoint(endpoint);
    }
    builder.build().expect("Failed to build the raw S3 client")
}

// ==================== UploadPartCopy on R2 ====================
//
// Each case pins one behavior the registry's part copy relies on.

/// Signed URLs for these cases expire after this, as the registry's do.
const SIGNED_URL_EXPIRY: std::time::Duration = std::time::Duration::from_mins(15);

/// `object_store`'s encoding of an S3 key in a request path.
const PATH_ENCODE_SET: percent_encoding::AsciiSet = percent_encoding::NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~')
    .remove(b'/');

/// Presigns an `UploadPartCopy` of part `number`, with `signed` copy headers.
async fn sign_part_copy(
    s3: &object_store::aws::AmazonS3,
    destination: &Path,
    upload_id: &str,
    number: usize,
    signed: &[(&'static str, String)],
    expires_in: std::time::Duration,
) -> object_store::signer::Url {
    use object_store::signer::{HeaderName, HeaderValue, Method, SignedUrlOptions, Signer as _};

    let mut options = SignedUrlOptions::default().with_query([
        ("partNumber", number.to_string()),
        ("uploadId", upload_id.to_owned()),
    ]);
    for (name, value) in signed {
        options = options.with_signed_header(
            HeaderName::from_static(name),
            HeaderValue::from_str(value).expect("Invalid header value"),
        );
    }
    s3.signed_url_opts(Method::PUT, destination, expires_in, &options)
        .await
        .expect("Failed to sign a part copy")
}

/// Sends a presigned `UploadPartCopy` with `sent` copy headers, and returns its status and body.
async fn send_part_copy(
    client: &reqwest::Client,
    url: &object_store::signer::Url,
    sent: &[(&'static str, String)],
) -> (u16, String) {
    let mut request = client.put(url.as_str());
    for (name, value) in sent {
        request = request.header(*name, value);
    }
    let response = request.send().await.expect("Failed to send a part copy");
    let status = response.status().as_u16();
    let body = response.text().await.expect("Failed to read a part copy");
    (status, body)
}

/// A part copy signed with the copy headers it sends.
async fn part_copy(
    client: &reqwest::Client,
    s3: &object_store::aws::AmazonS3,
    destination: &Path,
    upload_id: &str,
    number: usize,
    headers: &[(&'static str, String)],
) -> (u16, String) {
    let url = sign_part_copy(
        s3,
        destination,
        upload_id,
        number,
        headers,
        SIGNED_URL_EXPIRY,
    )
    .await;
    send_part_copy(client, &url, headers).await
}

/// The copy headers for `range` of `source`.
fn copy_headers(source: &Path, range: &str) -> Vec<(&'static str, String)> {
    let bucket = std::env::var(BUCKET).expect("Bucket is set");
    let key = percent_encoding::utf8_percent_encode(source.as_ref(), &PATH_ENCODE_SET);
    vec![
        ("x-amz-copy-source", format!("{bucket}/{key}")),
        ("x-amz-copy-source-range", range.to_owned()),
    ]
}

/// The `ETag` in a `CopyPartResult` body, which must not be an `<Error>`.
fn copied_e_tag(body: &str) -> String {
    assert!(body.contains("<CopyPartResult"), "{body}");
    assert!(!body.contains("<Error>"), "{body}");
    body.split_once("<ETag>")
        .and_then(|(_before, rest)| rest.split_once("</ETag>"))
        .map(|(e_tag, _after)| e_tag.replace("&quot;", "\""))
        .expect("A part copy answers with an ETag")
}

/// `len` bytes in a cycle of a prime length, so no range boundary lines up
/// with the cycle and a misplaced range shows.
fn varied_bytes(len: usize) -> Bytes {
    (0..251).cycle().take(len).collect::<Vec<u8>>().into()
}

/// A presigned `UploadPartCopy` works only with both copy headers signed, as they
/// were sent, before it expires; and a mount above one part copies by parts.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a scratch S3 bucket and credentials"]
async fn s3_presigned_part_copy() {
    use object_store::multipart::{MultipartStore as _, PartId};

    let Some(path) = run_path() else {
        return;
    };
    let s3 = raw_store();
    let client = reqwest::Client::new();
    let source = Path::from(format!("{path}/part-copy/source"));
    let destination = Path::from(format!("{path}/part-copy/destination"));
    let data = varied_bytes(4096);
    s3.put(&source, data.clone().into())
        .await
        .expect("Failed to put the source");
    let upload_id = s3
        .create_multipart(&destination)
        .await
        .expect("Failed to open the destination");
    let headers = copy_headers(&source, "bytes=0-4095");

    let unsigned = sign_part_copy(&s3, &destination, &upload_id, 1, &[], SIGNED_URL_EXPIRY).await;
    let (status, body) = send_part_copy(&client, &unsigned, &headers).await;
    assert_eq!(status, 403, "unsigned copy headers: {body}");
    assert!(
        body.contains("<Code>SignatureDoesNotMatch</Code>"),
        "{body}"
    );

    let signed = sign_part_copy(
        &s3,
        &destination,
        &upload_id,
        1,
        &headers,
        SIGNED_URL_EXPIRY,
    )
    .await;
    let (status, body) =
        send_part_copy(&client, &signed, &copy_headers(&source, "bytes=0-2047")).await;
    assert_eq!(status, 403, "an altered copy header: {body}");
    assert!(
        body.contains("<Code>SignatureDoesNotMatch</Code>"),
        "{body}"
    );

    let expiring = sign_part_copy(
        &s3,
        &destination,
        &upload_id,
        1,
        &headers,
        std::time::Duration::from_secs(1),
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let (status, body) = send_part_copy(&client, &expiring, &headers).await;
    assert_eq!(status, 403, "an expired URL: {body}");
    assert!(body.contains("<Code>ExpiredRequest</Code>"), "{body}");

    let (status, body) = part_copy(&client, &s3, &destination, &upload_id, 1, &headers).await;
    assert_eq!(status, 200, "{body}");
    let part = PartId {
        content_id: copied_e_tag(&body),
    };
    s3.complete_multipart(&destination, &upload_id, vec![part])
        .await
        .expect("Failed to complete the copy");
    let copied = s3
        .get(&destination)
        .await
        .expect("Failed to get the copy")
        .bytes()
        .await
        .expect("Failed to read the copy");
    assert_eq!(copied, data, "the copy holds the source's bytes");
    for key in [&source, &destination] {
        s3.delete(key)
            .await
            .expect("Failed to delete a copy object");
    }

    mount_above_one_part(&path).await;
}

/// A mount of a blob above one part copies it by presigned parts.
async fn mount_above_one_part(path: &str) {
    let storage = build_storage(path, 3600);
    let project = PROJECT.parse().expect("Invalid project UUID");
    let other = OTHER_PROJECT.parse().expect("Invalid project UUID");
    let blob = varied_bytes(16 * 1024 * 1024 + 1024);
    let upload = storage
        .start_upload(&project)
        .await
        .expect("Failed to start upload");
    storage
        .append_upload(&upload, blob.clone())
        .await
        .expect("Failed to append upload");
    let digest = storage
        .complete_upload(&upload, &Digest::from_sha256_bytes(&blob))
        .await
        .expect("Failed to complete upload");
    assert!(
        storage
            .mount_blob(&project, &other, &digest)
            .await
            .expect("Failed to mount blob"),
        "the mount succeeds"
    );
    let (mounted, _size) = storage
        .get_blob(&other, &digest)
        .await
        .expect("Failed to get the mounted blob");
    assert!(mounted == blob, "the mounted blob holds the source's bytes");

    for repository in [&project, &other] {
        storage
            .delete_blob(repository, &digest)
            .await
            .expect("Failed to delete blob");
    }
}

/// A copy range that ends past the source is clamped to a short part, and one
/// that starts past it is 416 `InvalidRange`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a scratch S3 bucket and credentials"]
async fn s3_part_copy_range_past_the_source() {
    use object_store::multipart::{MultipartStore as _, PartId};

    let Some(path) = run_path() else {
        return;
    };
    let s3 = raw_store();
    let client = reqwest::Client::new();
    let source = Path::from(format!("{path}/part-copy-range/source"));
    let destination = Path::from(format!("{path}/part-copy-range/destination"));
    let data = varied_bytes(1024);
    s3.put(&source, data.clone().into())
        .await
        .expect("Failed to put the source");
    let upload_id = s3
        .create_multipart(&destination)
        .await
        .expect("Failed to open the destination");

    let (status, body) = part_copy(
        &client,
        &s3,
        &destination,
        &upload_id,
        1,
        &copy_headers(&source, "bytes=4096-8191"),
    )
    .await;
    assert_eq!(status, 416, "{body}");
    assert!(body.contains("<Code>InvalidRange</Code>"), "{body}");

    let (status, body) = part_copy(
        &client,
        &s3,
        &destination,
        &upload_id,
        1,
        &copy_headers(&source, "bytes=512-2047"),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let part = PartId {
        content_id: copied_e_tag(&body),
    };
    s3.complete_multipart(&destination, &upload_id, vec![part])
        .await
        .expect("Failed to complete the copy");
    let copied = s3
        .get(&destination)
        .await
        .expect("Failed to get the copy")
        .bytes()
        .await
        .expect("Failed to read the copy");
    assert_eq!(
        copied,
        data.slice(512..),
        "the part is clamped to the source"
    );

    for key in [&source, &destination] {
        s3.delete(key)
            .await
            .expect("Failed to delete a copy object");
    }
}

/// A copy whose source is deleted while its parts copy still completes, from a
/// snapshot of the source.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a scratch S3 bucket and credentials"]
async fn s3_part_copy_of_a_source_deleted_mid_flight() {
    use object_store::multipart::{MultipartStore as _, PartId};
    use object_store::signer::{Method, Signer as _};

    const PART: usize = 16 * 1024 * 1024;
    const PARTS: usize = 8;

    let Some(path) = run_path() else {
        return;
    };
    let s3 = raw_store();
    let source = Path::from(format!("{path}/part-copy-deleted/source"));
    let destination = Path::from(format!("{path}/part-copy-deleted/destination"));
    let data = varied_bytes(PART * PARTS);
    let mut write = object_store::WriteMultipart::new_with_chunk_size(
        s3.put_multipart(&source)
            .await
            .expect("Failed to open the source"),
        PART,
    );
    write.write(&data);
    write.finish().await.expect("Failed to write the source");
    let upload_id = s3
        .create_multipart(&destination)
        .await
        .expect("Failed to open the destination");

    // One client, warmed by a first request, so every copy reaches the store
    // before the delete does
    let client = reqwest::Client::new();
    let head = s3
        .signed_url(Method::HEAD, &source, SIGNED_URL_EXPIRY)
        .await
        .expect("Failed to sign a HEAD of the source");
    let warmed = client
        .head(head.as_str())
        .send()
        .await
        .expect("Failed to HEAD the source");
    assert_eq!(warmed.status().as_u16(), 200, "the source is there");

    let copies = futures::future::join_all((1..=PARTS).map(|number| {
        let first = (number - 1) * PART;
        let headers = copy_headers(&source, &format!("bytes={first}-{}", first + PART - 1));
        let client = &client;
        let s3 = &s3;
        let destination = &destination;
        let upload_id = &upload_id;
        async move { part_copy(client, s3, destination, upload_id, number, &headers).await }
    }));
    let delete = async {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        s3.delete(&source)
            .await
            .expect("Failed to delete the source");
    };
    let (answers, ()) = tokio::join!(copies, delete);

    let parts = answers
        .iter()
        .map(|(status, body)| {
            assert_eq!(*status, 200, "{body}");
            PartId {
                content_id: copied_e_tag(body),
            }
        })
        .collect();
    s3.complete_multipart(&destination, &upload_id, parts)
        .await
        .expect("Failed to complete the copy");
    let copy = s3
        .get(&destination)
        .await
        .expect("Failed to get the copy")
        .bytes()
        .await
        .expect("Failed to read the copy");
    assert!(copy == data, "the copy holds the deleted source's bytes");

    s3.delete(&destination)
        .await
        .expect("Failed to delete the copy");
}
