//! S3-compatible replica backend (AWS S3, or R2/MinIO via `endpoint`).
//!
//! Construction mirrors the existing patterns in
//! `lib/bencher_schema/src/context/database.rs` (static credentials via
//! `aws_credential_types::Credentials::new`) with an optional endpoint
//! override for S3-compatible services (which also forces path-style
//! addressing).
//!
//! Error variants wrap the concrete `SdkError<OperationError>` types (never
//! `String`, never `Box<dyn Error>`), one variant per operation. Native
//! missing-object errors (`NoSuchKey`) are mapped to
//! `StorageError::NotFound` by the callers in this module, not surfaced as
//! `S3Error`.

use aws_credential_types::provider::SharedCredentialsProvider;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::operation::abort_multipart_upload::AbortMultipartUploadError;
use aws_sdk_s3::operation::complete_multipart_upload::CompleteMultipartUploadError;
use aws_sdk_s3::operation::create_multipart_upload::CreateMultipartUploadError;
use aws_sdk_s3::operation::delete_object::DeleteObjectError;
use aws_sdk_s3::operation::delete_objects::DeleteObjectsError;
use aws_sdk_s3::operation::get_object::GetObjectError;
use aws_sdk_s3::operation::list_multipart_uploads::ListMultipartUploadsError;
use aws_sdk_s3::operation::list_objects_v2::ListObjectsV2Error;
use aws_sdk_s3::operation::list_objects_v2::builders::ListObjectsV2FluentBuilder;
use aws_sdk_s3::operation::put_object::PutObjectError;
use aws_sdk_s3::operation::upload_part::UploadPartError;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart, Delete, ObjectIdentifier};
use bytes::Bytes;
use slog::Logger;

use crate::storage::StorageError;

/// The S3 minimum part size for all but the last part of a multipart
/// upload: writes are buffered until at least this many bytes are pending.
const MIN_PART_BYTES: usize = 5 * 1024 * 1024;

/// The S3 maximum part number for a multipart upload.
const MAX_PART_NUMBER: i32 = 10_000;

/// The S3 maximum number of objects per `DeleteObjects` request.
const DELETE_BATCH_SIZE: usize = 1_000;

/// The region assumed when none is configured (endpoint-only targets like
/// `MinIO` still require a signing region).
const DEFAULT_REGION: &str = "us-east-1";

/// S3-compatible backend: bucket plus optional key prefix.
pub struct S3Storage {
    client: aws_sdk_s3::Client,
    bucket: String,
    /// Key prefix (no leading or trailing slash); empty for bucket root.
    prefix: String,
    /// Test-only `ListObjectsV2` page-size cap, so the continuation-token
    /// pagination loop can be exercised against a small object count.
    #[cfg(feature = "testing")]
    max_keys: Option<i32>,
}

#[derive(Debug, thiserror::Error)]
pub enum S3Error {
    #[error("Failed to put object ({key}): {error}")]
    Put {
        key: String,
        error: SdkError<PutObjectError>,
    },
    #[error("Failed to get object ({key}): {error}")]
    Get {
        key: String,
        error: SdkError<GetObjectError>,
    },
    #[error("Failed to read object body ({key}): {error}")]
    Body {
        key: String,
        error: aws_sdk_s3::primitives::ByteStreamError,
    },
    #[error("Failed to list objects ({prefix}): {error}")]
    List {
        prefix: String,
        error: SdkError<ListObjectsV2Error>,
    },
    #[error(
        "Truncated listing without a continuation token ({prefix}); the listing would be silently short"
    )]
    TruncatedList { prefix: String },
    #[error("Failed to list multipart uploads ({prefix}): {error}")]
    ListMultipart {
        prefix: String,
        error: SdkError<ListMultipartUploadsError>,
    },
    #[error(
        "Truncated multipart-upload listing without a continuation marker ({prefix}); the sweep would be silently short"
    )]
    TruncatedMultipartList { prefix: String },
    #[error("Failed to delete object ({key}): {error}")]
    Delete {
        key: String,
        error: SdkError<DeleteObjectError>,
    },
    #[error("Failed to batch delete objects ({prefix}): {error}")]
    DeleteBatch {
        prefix: String,
        error: SdkError<DeleteObjectsError>,
    },
    #[error("Failed to build batch delete request ({prefix}): {error}")]
    DeleteBatchBuild {
        prefix: String,
        error: aws_sdk_s3::error::BuildError,
    },
    #[error("Batch delete reported a per-key failure ({prefix}, key {key}): {code} {message}")]
    DeleteBatchKey {
        prefix: String,
        key: String,
        code: String,
        message: String,
    },
    #[error("Failed to create multipart upload ({key}): {error}")]
    CreateMultipart {
        key: String,
        error: SdkError<CreateMultipartUploadError>,
    },
    #[error("Failed to upload part {part} ({key}): {error}")]
    UploadPart {
        key: String,
        part: i32,
        error: SdkError<UploadPartError>,
    },
    #[error("Failed to complete multipart upload ({key}): {error}")]
    CompleteMultipart {
        key: String,
        error: SdkError<CompleteMultipartUploadError>,
    },
    #[error("Failed to abort multipart upload ({key}): {error}")]
    AbortMultipart {
        key: String,
        error: SdkError<AbortMultipartUploadError>,
    },
    #[error("Multipart upload part count overflow ({key})")]
    PartCountOverflow { key: String },
    #[error("Multipart upload id missing from response ({key})")]
    MissingUploadId { key: String },
    #[error("ETag missing from upload part {part} response ({key})")]
    MissingETag { key: String, part: i32 },
}

impl S3Storage {
    /// Build a client from static credentials with an optional endpoint
    /// override (R2/MinIO) and optional region (defaults to `us-east-1` when
    /// none is given, e.g. endpoint-only targets).
    #[must_use]
    pub fn new(
        bucket: String,
        prefix: Option<String>,
        endpoint: Option<String>,
        region: Option<String>,
        access_key_id: String,
        secret_access_key: &str,
    ) -> Self {
        let credentials = aws_credential_types::Credentials::new(
            access_key_id,
            secret_access_key,
            None,
            None,
            "bencher_replica",
        );
        let region = Region::new(region.unwrap_or_else(|| DEFAULT_REGION.to_owned()));
        let mut config = aws_sdk_s3::Config::builder()
            .credentials_provider(SharedCredentialsProvider::new(credentials))
            .region(region);
        if let Some(endpoint) = endpoint {
            // S3-compatible services (MinIO, R2) are addressed by URL, not
            // by virtual-hosted bucket subdomains.
            config = config.endpoint_url(endpoint).force_path_style(true);
        }
        let client = aws_sdk_s3::Client::from_conf(config.build());
        Self {
            client,
            bucket,
            prefix: normalize_prefix(prefix),
            #[cfg(feature = "testing")]
            max_keys: None,
        }
    }

    /// Test-only: cap the `ListObjectsV2` page size so a modest object count
    /// still spans several continuation-token round-trips.
    #[cfg(feature = "testing")]
    pub fn set_max_keys(&mut self, max_keys: i32) {
        self.max_keys = Some(max_keys);
    }

    pub(crate) async fn put(&self, key: &str, bytes: Bytes) -> Result<(), StorageError> {
        crate::storage::validate_key(key)?;
        let object_key = self.object_key(key);
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .body(ByteStream::from(bytes))
            .send()
            .await
            .map_err(|error| S3Error::Put {
                key: object_key,
                error,
            })?;
        Ok(())
    }

    pub(crate) async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
        crate::storage::validate_key(key)?;
        let object_key = self.object_key(key);
        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .send()
            .await
            .map_err(|error| map_get_error(key, &object_key, error))?;
        let aggregated = response
            .body
            .collect()
            .await
            .map_err(|error| S3Error::Body {
                key: object_key,
                error,
            })?;
        Ok(aggregated.into_bytes())
    }

    pub(crate) async fn get_stream(
        &self,
        key: &str,
    ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, StorageError> {
        crate::storage::validate_key(key)?;
        let object_key = self.object_key(key);
        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .send()
            .await
            .map_err(|error| map_get_error(key, &object_key, error))?;
        Ok(Box::new(response.body.into_async_read()))
    }

    pub(crate) async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        let full_prefix = self.full_prefix(prefix);
        let root = self.root_prefix();
        let mut keys = Vec::new();
        let mut continuation: Option<String> = None;
        loop {
            let mut request = self
                .client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(&full_prefix);
            if let Some(token) = &continuation {
                request = request.continuation_token(token);
            }
            request = self.with_page_size(request);
            let response = request.send().await.map_err(|error| S3Error::List {
                prefix: full_prefix.clone(),
                error,
            })?;
            for object in response.contents() {
                if let Some(key) = object.key()
                    && let Some(stripped) = key.strip_prefix(root.as_str())
                {
                    keys.push(stripped.to_owned());
                }
            }
            match next_list_page(response.is_truncated(), response.next_continuation_token()) {
                ListPage::Continue(token) => continuation = Some(token),
                ListPage::Done => break,
                ListPage::Truncated => {
                    return Err(S3Error::TruncatedList {
                        prefix: full_prefix,
                    }
                    .into());
                },
            }
        }
        keys.sort();
        Ok(keys)
    }

    pub(crate) async fn list_dirs(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        let full_prefix = self.full_prefix(&normalize_dir_prefix(prefix));
        let mut dirs = Vec::new();
        let mut continuation: Option<String> = None;
        loop {
            let mut request = self
                .client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(&full_prefix)
                .delimiter("/");
            if let Some(token) = &continuation {
                request = request.continuation_token(token);
            }
            request = self.with_page_size(request);
            let response = request.send().await.map_err(|error| S3Error::List {
                prefix: full_prefix.clone(),
                error,
            })?;
            for common_prefix in response.common_prefixes() {
                // A common prefix is `<full_prefix><component>/`; return the
                // bare component.
                if let Some(prefixed) = common_prefix.prefix()
                    && let Some(component) = prefixed
                        .strip_prefix(full_prefix.as_str())
                        .and_then(|component| component.strip_suffix('/'))
                    && !component.is_empty()
                {
                    dirs.push(component.to_owned());
                }
            }
            match next_list_page(response.is_truncated(), response.next_continuation_token()) {
                ListPage::Continue(token) => continuation = Some(token),
                ListPage::Done => break,
                ListPage::Truncated => {
                    return Err(S3Error::TruncatedList {
                        prefix: full_prefix,
                    }
                    .into());
                },
            }
        }
        dirs.sort();
        Ok(dirs)
    }

    pub(crate) async fn delete(&self, key: &str) -> Result<(), StorageError> {
        crate::storage::validate_key(key)?;
        let object_key = self.object_key(key);
        // S3 DeleteObject on a missing key succeeds, giving idempotency
        // naturally.
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .send()
            .await
            .map_err(|error| S3Error::Delete {
                key: object_key,
                error,
            })?;
        Ok(())
    }

    pub(crate) async fn delete_prefix(&self, prefix: &str) -> Result<(), StorageError> {
        let keys = self.list(prefix).await?;
        for chunk in keys.chunks(DELETE_BATCH_SIZE) {
            let mut objects = Vec::with_capacity(chunk.len());
            for key in chunk {
                let identifier = ObjectIdentifier::builder()
                    .key(self.object_key(key))
                    .build()
                    .map_err(|error| S3Error::DeleteBatchBuild {
                        prefix: prefix.to_owned(),
                        error,
                    })?;
                objects.push(identifier);
            }
            let delete = Delete::builder()
                .set_objects(Some(objects))
                .build()
                .map_err(|error| S3Error::DeleteBatchBuild {
                    prefix: prefix.to_owned(),
                    error,
                })?;
            let response = self
                .client
                .delete_objects()
                .bucket(&self.bucket)
                .delete(delete)
                .send()
                .await
                .map_err(|error| S3Error::DeleteBatch {
                    prefix: self.full_prefix(prefix),
                    error,
                })?;
            // S3 reports per-key failures inside an HTTP 200 response; a
            // swallowed one would make prune claim success while objects
            // (possibly a snapshot body without its snapshot.json) remain.
            if let Some(first) = response.errors().first() {
                return Err(S3Error::DeleteBatchKey {
                    prefix: self.full_prefix(prefix),
                    key: first.key().unwrap_or_default().to_owned(),
                    code: first.code().unwrap_or_default().to_owned(),
                    message: first.message().unwrap_or_default().to_owned(),
                }
                .into());
            }
        }
        Ok(())
    }

    pub(crate) async fn start_multipart(&self, key: &str) -> Result<S3Multipart, StorageError> {
        crate::storage::validate_key(key)?;
        let object_key = self.object_key(key);
        let response = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(&object_key)
            .send()
            .await
            .map_err(|error| S3Error::CreateMultipart {
                key: object_key.clone(),
                error,
            })?;
        let upload_id = response
            .upload_id()
            .ok_or_else(|| S3Error::MissingUploadId {
                key: object_key.clone(),
            })?
            .to_owned();
        Ok(S3Multipart {
            client: self.client.clone(),
            bucket: self.bucket.clone(),
            key: object_key,
            upload_id,
            part_number: 0,
            buffer: Vec::new(),
            completed: Vec::new(),
        })
    }

    /// Best-effort sweep of crash-orphaned incomplete multipart uploads under
    /// the configured prefix. A process killed (SIGKILL/OOM) between
    /// `create_multipart_upload` and completion loses the upload id (held only
    /// in memory), and the incomplete upload then accrues storage cost until
    /// aborted. This reconciles by listing every incomplete upload scoped to
    /// the prefix and aborting it. Per-upload failures are logged and
    /// swallowed; a listing failure stops the sweep with a log line. Never
    /// fails the caller. NOT wired into the sync engine here: the orchestrator
    /// invokes it at startup.
    ///
    /// Refuses to run without a configured path prefix: with an empty prefix
    /// the listing is BUCKET-WIDE, and a shared bucket (database backups, OCI
    /// storage, another tenant) would have its in-flight multipart uploads
    /// aborted by every replica boot. Rely on a bucket lifecycle rule instead
    /// in that layout.
    pub(crate) async fn abort_incomplete_uploads(&self, log: &Logger) {
        if self.prefix.is_empty() {
            slog::warn!(
                log,
                "No S3 path prefix configured; skipping the incomplete multipart upload sweep \
                 (a bucket-wide sweep could abort other systems' uploads). \
                 Configure a bucket lifecycle rule to reap crash-orphaned uploads."
            );
            return;
        }
        let uploads = match self.list_incomplete_uploads().await {
            Ok(uploads) => uploads,
            Err(error) => {
                slog::warn!(log, "Failed to list incomplete multipart uploads; sweep skipped";
                    "error" => %error);
                return;
            },
        };
        let found = uploads.len();
        let mut aborted = 0usize;
        for (key, upload_id) in uploads {
            match abort_multipart(&self.client, &self.bucket, &key, &upload_id).await {
                Ok(()) => {
                    aborted += 1;
                    slog::info!(log, "Aborted crash-orphaned multipart upload";
                        "key" => key.as_str(), "upload_id" => upload_id.as_str());
                },
                Err(error) => {
                    slog::warn!(log, "Failed to abort incomplete multipart upload; continuing";
                        "key" => key.as_str(), "upload_id" => upload_id.as_str(), "error" => %error);
                },
            }
        }
        slog::info!(log, "Incomplete multipart upload sweep complete";
            "found" => found, "aborted" => aborted);
    }

    /// Every incomplete multipart upload under the configured prefix as
    /// `(object_key, upload_id)` pairs, paginated with the key-marker /
    /// upload-id-marker cursor and the same truncation guard as [`Self::list`]
    /// (a truncated page with no continuation marker is an error, never a
    /// silently short listing).
    async fn list_incomplete_uploads(&self) -> Result<Vec<(String, String)>, S3Error> {
        let prefix = self.root_prefix();
        let mut uploads = Vec::new();
        let mut key_marker: Option<String> = None;
        let mut upload_id_marker: Option<String> = None;
        loop {
            // Deliberately NO server-side `prefix` parameter: prefix
            // filtering on `ListMultipartUploads` is unreliable across
            // S3-compatible servers (MinIO matches the prefix only against
            // exact object keys, so a directory prefix returns nothing).
            // List everything and filter client-side; the caller-enforced
            // non-empty prefix keeps the ABORT scoped to our uploads, and
            // incomplete uploads are a small set.
            let mut request = self.client.list_multipart_uploads().bucket(&self.bucket);
            if let Some(marker) = &key_marker {
                request = request.key_marker(marker);
            }
            if let Some(marker) = &upload_id_marker {
                request = request.upload_id_marker(marker);
            }
            let response = request
                .send()
                .await
                .map_err(|error| S3Error::ListMultipart {
                    prefix: prefix.clone(),
                    error,
                })?;
            for upload in response.uploads() {
                if let (Some(key), Some(upload_id)) = (upload.key(), upload.upload_id())
                    && key.starts_with(&prefix)
                {
                    uploads.push((key.to_owned(), upload_id.to_owned()));
                }
            }
            match next_upload_page(
                response.is_truncated(),
                response.next_key_marker(),
                response.next_upload_id_marker(),
            ) {
                UploadPage::Continue {
                    key_marker: next_key,
                    upload_id_marker: next_upload_id,
                } => {
                    key_marker = Some(next_key);
                    upload_id_marker = next_upload_id;
                },
                UploadPage::Done => break,
                UploadPage::Truncated => {
                    return Err(S3Error::TruncatedMultipartList { prefix });
                },
            }
        }
        Ok(uploads)
    }

    /// Test-only: how many incomplete multipart uploads exist under the
    /// configured prefix (a listing failure reports 0).
    #[cfg(feature = "testing")]
    pub async fn incomplete_upload_count(&self) -> usize {
        self.list_incomplete_uploads()
            .await
            .map(|uploads| uploads.len())
            .unwrap_or_default()
    }

    /// The full object key for a caller-relative key.
    fn object_key(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_owned()
        } else {
            format!("{}/{key}", self.prefix)
        }
    }

    /// The configured root as a listing prefix (`""` or `"<prefix>/"`); also
    /// what gets stripped from listed keys.
    fn root_prefix(&self) -> String {
        if self.prefix.is_empty() {
            String::new()
        } else {
            format!("{}/", self.prefix)
        }
    }

    /// A caller-relative listing prefix scoped under the configured root.
    fn full_prefix(&self, prefix: &str) -> String {
        format!("{}{prefix}", self.root_prefix())
    }

    /// Apply the test-only page-size cap to a `ListObjectsV2` request; a no-op
    /// in production builds.
    fn with_page_size(&self, request: ListObjectsV2FluentBuilder) -> ListObjectsV2FluentBuilder {
        #[cfg(feature = "testing")]
        if let Some(max_keys) = self.max_keys {
            return request.max_keys(max_keys);
        }
        request
    }
}

/// How a paginated `ListObjectsV2` loop proceeds from one response.
enum ListPage {
    /// More results remain; continue with this continuation token.
    Continue(String),
    /// The listing is complete.
    Done,
    /// The response was truncated but carried no usable continuation token:
    /// continuing is impossible and stopping would silently drop keys. The
    /// caller must raise an error naming the prefix.
    Truncated,
}

/// Decide how a paginated listing continues. `is_truncated == Some(true)` with
/// a non-empty token continues; not truncated is done; truncated with no
/// usable token is [`ListPage::Truncated`] (an error, never a short listing,
/// because the engine treats LIST as the source of truth: a missing key would
/// masquerade as a deleted object).
fn next_list_page(is_truncated: Option<bool>, next_token: Option<&str>) -> ListPage {
    if is_truncated == Some(true) {
        match next_token {
            Some(token) if !token.is_empty() => ListPage::Continue(token.to_owned()),
            _ => ListPage::Truncated,
        }
    } else {
        ListPage::Done
    }
}

/// How a paginated `ListMultipartUploads` loop proceeds from one response. The
/// cursor is a `(key_marker, upload_id_marker)` pair; the key marker is the
/// primary continuation token and the upload-id marker is only meaningful
/// alongside it.
enum UploadPage {
    /// More results remain; continue with these markers.
    Continue {
        key_marker: String,
        upload_id_marker: Option<String>,
    },
    /// The listing is complete.
    Done,
    /// Truncated but with no usable key marker: continuing is impossible and
    /// stopping would silently drop uploads. The caller must raise an error.
    Truncated,
}

/// Decide how a `ListMultipartUploads` listing continues, applying the same
/// truncation rigor as [`next_list_page`]: truncated with a key marker
/// continues; not truncated is done; truncated with no usable key marker is
/// [`UploadPage::Truncated`].
fn next_upload_page(
    is_truncated: Option<bool>,
    next_key_marker: Option<&str>,
    next_upload_id_marker: Option<&str>,
) -> UploadPage {
    if is_truncated == Some(true) {
        match next_key_marker {
            Some(key_marker) if !key_marker.is_empty() => UploadPage::Continue {
                key_marker: key_marker.to_owned(),
                upload_id_marker: next_upload_id_marker
                    .filter(|marker| !marker.is_empty())
                    .map(ToOwned::to_owned),
            },
            _ => UploadPage::Truncated,
        }
    } else {
        UploadPage::Done
    }
}

/// Streaming upload to S3 via multipart. Buffers writes internally until the
/// 5 MiB minimum part size is reached, then uploads parts sequentially.
/// Dropping without [`Self::finish`] leaves only an uncompleted multipart
/// upload: the object never becomes visible under its key.
pub struct S3Multipart {
    client: aws_sdk_s3::Client,
    bucket: String,
    key: String,
    upload_id: String,
    part_number: i32,
    buffer: Vec<u8>,
    completed: Vec<CompletedPart>,
}

impl S3Multipart {
    pub(crate) async fn write_part(&mut self, bytes: Bytes) -> Result<(), StorageError> {
        self.buffer.extend_from_slice(&bytes);
        if self.buffer.len() >= MIN_PART_BYTES {
            self.upload_buffered_part().await?;
        }
        Ok(())
    }

    pub(crate) async fn finish(self) -> Result<(), StorageError> {
        if self.completed.is_empty() && self.buffer.is_empty() {
            // S3 rejects CompleteMultipartUpload with zero parts: abort the
            // upload and store the zero-byte object with a plain PUT.
            let Self {
                client,
                bucket,
                key,
                upload_id,
                ..
            } = self;
            abort_multipart(&client, &bucket, &key, &upload_id).await?;
            client
                .put_object()
                .bucket(&bucket)
                .key(&key)
                .body(ByteStream::from_static(&[]))
                .send()
                .await
                .map_err(|error| S3Error::Put { key, error })?;
            return Ok(());
        }
        // Capture the abort coordinates before the fallible tail consumes
        // self, so ANY failure inside it (a final part upload, a part-count
        // overflow, or the completion call) aborts the upload instead of
        // leaking an uncompleted multipart upload that accrues storage cost
        // forever.
        let client = self.client.clone();
        let bucket = self.bucket.clone();
        let key = self.key.clone();
        let upload_id = self.upload_id.clone();
        if let Err(error) = self.complete().await {
            // Best-effort abort; the original error is always returned and the
            // abort's own failure is swallowed.
            drop(abort_multipart(&client, &bucket, &key, &upload_id).await);
            return Err(error);
        }
        Ok(())
    }

    /// The fallible completion tail: upload any final buffered part, then
    /// complete the multipart upload. Kept as a single guarded region so a
    /// future error path added here cannot forget the abort in
    /// [`Self::finish`].
    async fn complete(mut self) -> Result<(), StorageError> {
        if !self.buffer.is_empty() {
            // The final part may be smaller than the 5 MiB minimum.
            self.upload_buffered_part().await?;
        }
        let Self {
            client,
            bucket,
            key,
            upload_id,
            completed,
            ..
        } = self;
        let parts = CompletedMultipartUpload::builder()
            .set_parts(Some(completed))
            .build();
        client
            .complete_multipart_upload()
            .bucket(&bucket)
            .key(&key)
            .upload_id(&upload_id)
            .multipart_upload(parts)
            .send()
            .await
            .map_err(|error| S3Error::CompleteMultipart { key, error })?;
        Ok(())
    }

    pub(crate) async fn abort(self) -> Result<(), StorageError> {
        let Self {
            client,
            bucket,
            key,
            upload_id,
            ..
        } = self;
        abort_multipart(&client, &bucket, &key, &upload_id).await?;
        Ok(())
    }

    /// Upload the entire pending buffer as the next part.
    async fn upload_buffered_part(&mut self) -> Result<(), StorageError> {
        let part_number = self
            .part_number
            .checked_add(1)
            .filter(|part_number| *part_number <= MAX_PART_NUMBER)
            .ok_or_else(|| S3Error::PartCountOverflow {
                key: self.key.clone(),
            })?;
        let body = std::mem::take(&mut self.buffer);
        let response = self
            .client
            .upload_part()
            .bucket(&self.bucket)
            .key(&self.key)
            .upload_id(&self.upload_id)
            .part_number(part_number)
            .body(ByteStream::from(body))
            .send()
            .await
            .map_err(|error| S3Error::UploadPart {
                key: self.key.clone(),
                part: part_number,
                error,
            })?;
        let e_tag = response
            .e_tag()
            .ok_or_else(|| S3Error::MissingETag {
                key: self.key.clone(),
                part: part_number,
            })?
            .to_owned();
        self.part_number = part_number;
        self.completed.push(
            CompletedPart::builder()
                .part_number(part_number)
                .e_tag(e_tag)
                .build(),
        );
        Ok(())
    }
}

/// Map a `GetObject` failure: a missing key becomes
/// [`StorageError::NotFound`] (with the caller-relative key), anything else
/// wraps the original error.
fn map_get_error(key: &str, object_key: &str, error: SdkError<GetObjectError>) -> StorageError {
    if error
        .as_service_error()
        .is_some_and(GetObjectError::is_no_such_key)
    {
        StorageError::NotFound {
            key: key.to_owned(),
        }
    } else {
        S3Error::Get {
            key: object_key.to_owned(),
            error,
        }
        .into()
    }
}

async fn abort_multipart(
    client: &aws_sdk_s3::Client,
    bucket: &str,
    key: &str,
    upload_id: &str,
) -> Result<(), S3Error> {
    client
        .abort_multipart_upload()
        .bucket(bucket)
        .key(key)
        .upload_id(upload_id)
        .send()
        .await
        .map_err(|error| S3Error::AbortMultipart {
            key: key.to_owned(),
            error,
        })?;
    Ok(())
}

/// Normalize a configured prefix: no leading/trailing/doubled slashes;
/// `None` and `"/"` become the empty (bucket-root) prefix.
fn normalize_prefix(prefix: Option<String>) -> String {
    prefix
        .unwrap_or_default()
        .split('/')
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

/// Directory listings require a trailing slash on non-empty prefixes so S3
/// common prefixes align on whole components.
fn normalize_dir_prefix(prefix: &str) -> String {
    if prefix.is_empty() || prefix.ends_with('/') {
        prefix.to_owned()
    } else {
        format!("{prefix}/")
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn storage(prefix: Option<&str>, endpoint: Option<&str>) -> S3Storage {
        S3Storage::new(
            "test-bucket".to_owned(),
            prefix.map(ToOwned::to_owned),
            endpoint.map(ToOwned::to_owned),
            None,
            "test-access-key".to_owned(),
            "test-secret-key",
        )
    }

    #[test]
    fn prefix_is_normalized() {
        assert_eq!(storage(None, None).prefix, "", "None prefix");
        assert_eq!(storage(Some(""), None).prefix, "", "empty prefix");
        assert_eq!(storage(Some("/"), None).prefix, "", "slash-only prefix");
        assert_eq!(
            storage(Some("/replica//prod/"), None).prefix,
            "replica/prod",
            "leading, trailing, and doubled slashes are removed"
        );
    }

    #[test]
    fn object_key_joins_with_single_slash() {
        let scoped = storage(Some("replica"), None);
        assert_eq!(
            scoped.object_key("generations/g1/snapshot.json"),
            "replica/generations/g1/snapshot.json",
            "scoped key"
        );
        let root = storage(None, None);
        assert_eq!(
            root.object_key("generations/g1/snapshot.json"),
            "generations/g1/snapshot.json",
            "root key has no leading slash"
        );
    }

    #[test]
    fn full_prefix_scopes_listings_to_the_root() {
        let scoped = storage(Some("replica"), None);
        // An empty caller prefix must still scope to `replica/`, never
        // matching sibling prefixes like `replica2/`.
        assert_eq!(scoped.full_prefix(""), "replica/", "empty caller prefix");
        assert_eq!(
            scoped.full_prefix("generations/"),
            "replica/generations/",
            "nested caller prefix"
        );
        let root = storage(None, None);
        assert_eq!(root.full_prefix(""), "", "root empty prefix");
        assert_eq!(
            root.full_prefix("generations/"),
            "generations/",
            "root nested prefix"
        );
    }

    #[test]
    fn root_prefix_strips_listed_keys() {
        let scoped = storage(Some("replica"), None);
        let listed = "replica/generations/g1/snapshot.json";
        assert_eq!(
            listed.strip_prefix(scoped.root_prefix().as_str()),
            Some("generations/g1/snapshot.json"),
            "listed keys are returned caller-relative"
        );
    }

    #[test]
    fn dir_prefix_gets_trailing_slash() {
        assert_eq!(normalize_dir_prefix(""), "", "empty stays empty");
        assert_eq!(
            normalize_dir_prefix("generations"),
            "generations/",
            "missing slash is added"
        );
        assert_eq!(
            normalize_dir_prefix("generations/"),
            "generations/",
            "existing slash is kept"
        );
    }

    #[test]
    fn endpoint_override_builds_a_client() {
        // MinIO/R2-style construction: endpoint plus default region; the
        // client must build without panicking.
        let scoped = storage(Some("replica"), Some("http://localhost:9000"));
        assert_eq!(scoped.bucket, "test-bucket", "bucket is stored");
    }

    #[test]
    fn next_list_page_decides_continue_done_or_error() {
        assert!(
            matches!(
                next_list_page(Some(true), Some("token")),
                ListPage::Continue(token) if token == "token"
            ),
            "truncated with a token must continue"
        );
        assert!(
            matches!(next_list_page(Some(false), None), ListPage::Done),
            "not truncated must be done"
        );
        assert!(
            matches!(next_list_page(None, None), ListPage::Done),
            "absent truncation flag must be done"
        );
        assert!(
            matches!(next_list_page(Some(false), Some("token")), ListPage::Done),
            "a stray token without truncation is still done"
        );
        assert!(
            matches!(next_list_page(Some(true), None), ListPage::Truncated),
            "truncated with no token must be an error"
        );
        assert!(
            matches!(next_list_page(Some(true), Some("")), ListPage::Truncated),
            "truncated with an empty token must be an error"
        );
    }

    #[test]
    fn next_upload_page_decides_continue_done_or_error() {
        assert!(
            matches!(
                next_upload_page(Some(true), Some("key"), Some("uid")),
                UploadPage::Continue { key_marker, upload_id_marker }
                    if key_marker == "key" && upload_id_marker.as_deref() == Some("uid")
            ),
            "truncated with both markers must continue with both"
        );
        assert!(
            matches!(
                next_upload_page(Some(true), Some("key"), None),
                UploadPage::Continue { key_marker, upload_id_marker }
                    if key_marker == "key" && upload_id_marker.is_none()
            ),
            "truncated with only a key marker continues without an upload-id marker"
        );
        assert!(
            matches!(next_upload_page(Some(false), None, None), UploadPage::Done),
            "not truncated must be done"
        );
        assert!(
            matches!(next_upload_page(None, None, None), UploadPage::Done),
            "absent truncation flag must be done"
        );
        assert!(
            matches!(
                next_upload_page(Some(true), None, Some("uid")),
                UploadPage::Truncated
            ),
            "truncated with no key marker must be an error"
        );
        assert!(
            matches!(
                next_upload_page(Some(true), Some(""), None),
                UploadPage::Truncated
            ),
            "truncated with an empty key marker must be an error"
        );
    }

    #[tokio::test]
    async fn object_methods_reject_escaping_keys() {
        // Validation happens before any network call, so a fake client is
        // fine: an escaping key never reaches S3.
        let s3 = storage(Some("replica"), None);
        for key in ["../escape", "/leading", "gen/../escape", "gen//doubled"] {
            assert!(
                matches!(
                    s3.put(key, Bytes::from_static(b"x")).await,
                    Err(StorageError::InvalidKey { .. })
                ),
                "put({key}) must be InvalidKey before any request"
            );
            assert!(
                matches!(s3.get(key).await, Err(StorageError::InvalidKey { .. })),
                "get({key}) must be InvalidKey"
            );
            assert!(
                matches!(s3.delete(key).await, Err(StorageError::InvalidKey { .. })),
                "delete({key}) must be InvalidKey"
            );
            assert!(
                matches!(
                    s3.start_multipart(key).await,
                    Err(StorageError::InvalidKey { .. })
                ),
                "start_multipart({key}) must be InvalidKey"
            );
        }
    }
}
