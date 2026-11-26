//! A server-side copy of one S3 object to another key, by parallel presigned `UploadPartCopy`.
//!
//! The copy is three steps that a caller drives, so the caller can save the upload
//! ID between them: [`PartCopy::open`], [`PartCopy::copy_parts`], and
//! [`PartCopy::complete`]. No step aborts the upload: that is the caller's call.

use std::fmt;
use std::num::NonZeroU64;
use std::ops::Range;
use std::time::{Duration, Instant};

use futures::stream::{FuturesUnordered, StreamExt as _};
use http::header::{HeaderMap, HeaderName, HeaderValue};
use http::{Method, StatusCode};
use md5::{Digest as _, Md5};
use object_store::aws::AmazonS3;
use object_store::client::{
    HttpClient, HttpConnector as _, HttpErrorKind, HttpRequestBody, ReqwestConnector,
};
use object_store::multipart::{MultipartStore as _, PartId};
use object_store::signer::{SignedUrlOptions, Signer as _};
use object_store::{ClientOptions, MultipartId, ObjectStoreExt as _, path::Path};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;
use slog::Logger;
use thiserror::Error;

use crate::metrics::{self, StorageErrorCode, StorageOperation, StorageOutcome};
use crate::s3_error::{self, Answer, S3Error};
use crate::storage::{MAX_PARTS, OciStorageError, storage_error};

/// Most part copies in flight at once
const MAX_COPIES: usize = 16;

/// Most attempts at one part
const MAX_ATTEMPTS: u32 = 4;

/// The wait before a part's first retry, which doubles for each retry after it
const FIRST_BACKOFF: Duration = Duration::from_millis(100);

/// How long a presigned part copy stays valid
const SIGNED_URL_EXPIRY: Duration = Duration::from_mins(15);

/// A part grows in whole mebibytes when a copy would need more than [`MAX_PARTS`].
const MIB: u64 = 1024 * 1024;

/// `object_store`'s encoding of an S3 key in a request path, which it keeps
/// private: every character but an ASCII letter, a digit, or one of `-._~/`.
const PATH_ENCODE_SET: AsciiSet = NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~')
    .remove(b'/');

static COPY_SOURCE: HeaderName = HeaderName::from_static("x-amz-copy-source");
static COPY_SOURCE_RANGE: HeaderName = HeaderName::from_static("x-amz-copy-source-range");

/// The S3 backend's own handle, for the calls that the object store trait hides.
pub(crate) struct PartCopy {
    s3: AmazonS3,
    bucket: String,
    /// One client for every part copy, with the store's own client options
    client: HttpClient,
    log: Logger,
}

/// Why a copy of parts failed.
#[derive(Debug, Error)]
pub(crate) enum CopyError {
    /// The destination upload is gone: completed, or aborted.
    #[error("The upload the parts copy into is gone")]
    UploadGone,
    #[error(transparent)]
    Storage(#[from] OciStorageError),
}

impl From<CopyError> for OciStorageError {
    fn from(error: CopyError) -> Self {
        match error {
            CopyError::UploadGone => Self::Storage(error.to_string()),
            CopyError::Storage(error) => error,
        }
    }
}

/// How a completion ended.
#[derive(Debug)]
pub(crate) enum Completion {
    /// This call completed the upload, and the object's `ETag` matches its parts.
    Completed { e_tag: String },
    /// The upload was already complete, as after a lost answer or a retry, and the
    /// destination holds an object of the source's size.
    Found,
}

impl PartCopy {
    pub(crate) fn new(
        s3: AmazonS3,
        bucket: String,
        options: &ClientOptions,
        log: Logger,
    ) -> Result<Self, OciStorageError> {
        let client = ReqwestConnector::default()
            .connect(options)
            .map_err(|e| OciStorageError::Config(e.to_string()))?;
        Ok(Self {
            s3,
            bucket,
            client,
            log,
        })
    }

    /// Opens a multipart upload at `destination`, and returns its ID.
    pub(crate) async fn open(&self, destination: &Path) -> Result<MultipartId, OciStorageError> {
        metrics::time(
            StorageOperation::CreateMultipart,
            self.s3.create_multipart(destination),
        )
        .await
        .map_err(|e| storage_error(&e))
    }

    /// Copies every range of `source`, `size` bytes, into the upload, and returns the parts in order.
    ///
    /// Once a part fails its last attempt, no new part starts, and the copies in
    /// flight finish before the failure returns, so each records its answer.
    /// A destination upload that S3 no longer knows fails as [`CopyError::UploadGone`].
    pub(crate) async fn copy_parts(
        &self,
        source: &Path,
        destination: &Path,
        upload_id: &MultipartId,
        size: u64,
        part_size: NonZeroU64,
    ) -> Result<Vec<PartId>, CopyError> {
        let copy_source = self.copy_source(source)?;
        let mut ranges = (1..).zip(copy_ranges(size, part_size));
        let mut in_flight = FuturesUnordered::new();
        let mut parts = Vec::new();
        let mut failure = None;
        loop {
            while failure.is_none()
                && in_flight.len() < MAX_COPIES
                && let Some((number, range)) = ranges.next()
            {
                in_flight.push(self.copy_part(&copy_source, destination, upload_id, number, range));
            }
            let Some(copied) = in_flight.next().await else {
                break;
            };
            match copied {
                Ok(part) => parts.push(part),
                Err(error) => {
                    failure.get_or_insert(error);
                },
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
        parts.sort_unstable_by_key(|(number, _part)| *number);
        Ok(parts.into_iter().map(|(_number, part)| part).collect())
    }

    /// Completes the upload with its parts in order, and checks what it completed.
    ///
    /// A completion that S3 no longer knows was completed before, by a lost
    /// answer or a retry, so the destination must hold `size` bytes.
    pub(crate) async fn complete(
        &self,
        destination: &Path,
        upload_id: &MultipartId,
        parts: Vec<PartId>,
        size: u64,
    ) -> Result<Completion, OciStorageError> {
        let expected = multipart_e_tag(&parts).ok_or_else(|| {
            OciStorageError::Storage("A part's ETag is not an MD5 digest".to_owned())
        })?;
        let completed = metrics::time(
            StorageOperation::Complete,
            self.s3.complete_multipart(destination, upload_id, parts),
        )
        .await;
        if let Err(error) = &completed
            && is_no_such_upload(error)
        {
            let meta = metrics::time(StorageOperation::Head, self.s3.head(destination))
                .await
                .map_err(|e| storage_error(&e))?;
            return if meta.size == size {
                Ok(Completion::Found)
            } else {
                Err(OciStorageError::Storage(format!(
                    "{destination} holds {} bytes, not the {size} bytes copied",
                    meta.size
                )))
            };
        }
        let e_tag = completed
            .map_err(|e| storage_error(&e))?
            .e_tag
            .unwrap_or_default();
        if e_tag.trim_matches('"') == expected {
            Ok(Completion::Completed { e_tag })
        } else {
            Err(OciStorageError::Storage(format!(
                "Completed {destination} with ETag {e_tag}, not its parts' ETag {expected}"
            )))
        }
    }

    /// Aborts the upload, which never touches a completed object.
    pub(crate) async fn abort(
        &self,
        destination: &Path,
        upload_id: &MultipartId,
    ) -> Result<(), OciStorageError> {
        metrics::time(
            StorageOperation::Abort,
            self.s3.abort_multipart(destination, upload_id),
        )
        .await
        .map_err(|e| storage_error(&e))
    }

    /// Names `source` as a copy header does: its bucket, then its key as a request path encodes it.
    fn copy_source(&self, source: &Path) -> Result<HeaderValue, OciStorageError> {
        let encoded = utf8_percent_encode(source.as_ref(), &PATH_ENCODE_SET);
        HeaderValue::from_str(&format!("{}/{encoded}", self.bucket))
            .map_err(|e| OciStorageError::Config(format!("Invalid copy source: {e}")))
    }

    /// Copies one range as part `number`, retrying as S3 allows, and records each attempt.
    async fn copy_part(
        &self,
        copy_source: &HeaderValue,
        destination: &Path,
        upload_id: &MultipartId,
        number: usize,
        range: Range<u64>,
    ) -> Result<(usize, PartId), CopyError> {
        let mut headers = HeaderMap::new();
        headers.insert(COPY_SOURCE.clone(), copy_source.clone());
        headers.insert(COPY_SOURCE_RANGE.clone(), copy_range(&range)?);
        let mut attempt = 1;
        loop {
            let start = Instant::now();
            let copied = self.attempt(destination, upload_id, number, &headers).await;
            let outcome = match &copied {
                Ok(_part) => StorageOutcome::Ok,
                Err(failure) => StorageOutcome::Error(failure.code()),
            };
            metrics::record(StorageOperation::PartCopy, outcome, start.elapsed());
            let failure = match copied {
                Ok(part) => return Ok((number, part)),
                Err(failure) => failure,
            };
            slog::warn!(self.log, "OCI part copy failed";
                "operation" => "part_copy",
                "part" => number,
                "attempt" => attempt,
                "error" => %failure,
            );
            if failure.is_no_such_upload() {
                return Err(CopyError::UploadGone);
            }
            if attempt >= MAX_ATTEMPTS || !failure.retries() {
                return Err(OciStorageError::Storage(format!(
                    "Part copy of part {number} failed on attempt {attempt}: {failure}"
                ))
                .into());
            }
            tokio::time::sleep(backoff(attempt)).await;
            attempt += 1;
        }
    }

    /// One presigned `UploadPartCopy`, signed afresh, so a retry never sends an expired URL.
    async fn attempt(
        &self,
        destination: &Path,
        upload_id: &MultipartId,
        number: usize,
        headers: &HeaderMap,
    ) -> Result<PartId, Failure> {
        let mut options = SignedUrlOptions::default().with_query([
            ("partNumber", number.to_string()),
            ("uploadId", upload_id.clone()),
        ]);
        options.signed_headers = headers.clone();
        let url = self
            .s3
            .signed_url_opts(Method::PUT, destination, SIGNED_URL_EXPIRY, &options)
            .await
            .map_err(|e| Failure::Sign(s3_error::redact(&e)))?;

        let mut request = http::Request::new(HttpRequestBody::empty());
        *request.method_mut() = Method::PUT;
        *request.uri_mut() = url
            .as_str()
            .parse()
            .map_err(|_e| Failure::Sign("The signed URL is not a URI".to_owned()))?;
        *request.headers_mut() = headers.clone();

        // A transport failure keeps only its kind: an HTTP error's text can name
        // the presigned URL, which holds the access key ID and the signature.
        let response = self
            .client
            .execute(request)
            .await
            .map_err(|e| Failure::Transport(e.kind()))?;
        let status = response.status();
        let body = response
            .into_body()
            .bytes()
            .await
            .map_err(|e| Failure::Transport(e.kind()))?;
        if status != StatusCode::OK {
            return Err(Failure::Answer {
                status,
                error: S3Error::parse(&body),
            });
        }
        if let Some(error) = S3Error::parse(&body) {
            return Err(Failure::Answer {
                status,
                error: Some(error),
            });
        }
        quick_xml::de::from_reader::<_, CopyPartResult>(body.as_ref())
            .map(|result| PartId {
                content_id: result.e_tag,
            })
            .map_err(|_e| Failure::NoETag)
    }
}

/// The body of a successful `UploadPartCopy`.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CopyPartResult {
    e_tag: String,
}

/// Why one attempt at a part copy failed, holding nothing from the answer's body
/// but its S3 `Code` and `Message`.
enum Failure {
    /// The URL could not be signed.
    Sign(String),
    /// The store could not be reached, or stopped answering.
    Transport(HttpErrorKind),
    /// The store answered with a failure.
    Answer {
        status: StatusCode,
        error: Option<S3Error>,
    },
    /// The store answered 200 with no part `ETag`.
    NoETag,
}

impl Failure {
    /// Whether another attempt may succeed: a busy or failing store, a 200 with
    /// an `<Error>` body, an expired URL, or a connection that failed.
    fn retries(&self) -> bool {
        match self {
            Self::Sign(_) | Self::NoETag => false,
            Self::Transport(kind) => [
                HttpErrorKind::Timeout,
                HttpErrorKind::Connect,
                HttpErrorKind::Request,
                HttpErrorKind::Interrupted,
            ]
            .contains(kind),
            Self::Answer { status, error } => {
                *status == StatusCode::OK
                    || *status == StatusCode::TOO_MANY_REQUESTS
                    || status.is_server_error()
                    || (*status == StatusCode::FORBIDDEN
                        && error.as_ref().and_then(S3Error::code) == Some("ExpiredRequest"))
            },
        }
    }

    /// Whether S3 answered as it does for an upload it no longer knows: 404 `NoSuchUpload`.
    fn is_no_such_upload(&self) -> bool {
        let Self::Answer { status, error } = self else {
            return false;
        };
        *status == StatusCode::NOT_FOUND
            && error.as_ref().and_then(S3Error::code) == Some("NoSuchUpload")
    }

    fn code(&self) -> StorageErrorCode {
        match self {
            Self::Sign(_) | Self::NoETag => StorageErrorCode::Other,
            Self::Transport(kind) => {
                StorageErrorCode::of_transport(*kind).unwrap_or(StorageErrorCode::Other)
            },
            Self::Answer { status, error } => {
                StorageErrorCode::of_answer(Some(*status), error.as_ref())
            },
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sign(error) => write!(f, "{error}"),
            Self::Transport(kind) => {
                let failure = if *kind == HttpErrorKind::Timeout {
                    "the store timed out"
                } else if *kind == HttpErrorKind::Connect {
                    "the store could not be reached"
                } else {
                    "the connection to the store failed"
                };
                write!(f, "{failure}")
            },
            Self::Answer { status, error } => write!(
                f,
                "{}",
                s3_error::describe_error(Some(*status), error.as_ref())
            ),
            Self::NoETag => write!(f, "HTTP 200 OK without a part ETag"),
        }
    }
}

/// The byte ranges of a copy of `size` bytes: `part_size` each but a shorter
/// last, with the part grown to the smallest whole number of mebibytes that fits
/// the copy in [`MAX_PARTS`] when `part_size` does not.
pub(crate) fn copy_ranges(size: u64, part_size: NonZeroU64) -> Vec<Range<u64>> {
    let part_size = if size.div_ceil(part_size.get()) > MAX_PARTS {
        size.div_ceil(MAX_PARTS).next_multiple_of(MIB)
    } else {
        part_size.get()
    };
    let mut ranges = Vec::new();
    let mut start = 0;
    while start < size {
        let end = start.saturating_add(part_size).min(size);
        ranges.push(start..end);
        start = end;
    }
    ranges
}

/// A copy range header, whose last byte is inclusive.
fn copy_range(range: &Range<u64>) -> Result<HeaderValue, OciStorageError> {
    let last = range.end.saturating_sub(1);
    HeaderValue::from_str(&format!("bytes={}-{last}", range.start))
        .map_err(|e| OciStorageError::Storage(format!("Invalid copy range: {e}")))
}

/// The wait before retry `attempt`: a doubling step, cut by up to half at random
/// so parts that failed together do not retry together.
fn backoff(attempt: u32) -> Duration {
    let step = FIRST_BACKOFF.saturating_mul(2u32.saturating_pow(attempt.saturating_sub(1)));
    step.mul_f64(rand::random_range(0.5..=1.0))
}

/// The `ETag` S3 gives a multipart object: the MD5 of its parts' binary MD5s, then
/// `-` and the number of parts.
pub(crate) fn multipart_e_tag(parts: &[PartId]) -> Option<String> {
    let mut md5 = Md5::new();
    for part in parts {
        let digest = hex::decode(part.content_id.trim_matches('"')).ok()?;
        if digest.len() != Md5::output_size() {
            return None;
        }
        md5.update(digest);
    }
    Some(format!("{}-{}", hex::encode(md5.finalize()), parts.len()))
}

/// Whether a completion failed as S3 fails one it already completed: 404 `NoSuchUpload`.
pub(crate) fn is_no_such_upload(error: &object_store::Error) -> bool {
    answered_with(error, StatusCode::NOT_FOUND, "NoSuchUpload")
}

/// Whether a completion failed as S3 fails one that lists a part other than the
/// newest of its number: 400 `InvalidPart`.
pub(crate) fn is_invalid_part(error: &object_store::Error) -> bool {
    answered_with(error, StatusCode::BAD_REQUEST, "InvalidPart")
}

/// Whether S3 answered the call with `status` and the S3 `code`.
fn answered_with(error: &object_store::Error, status: StatusCode, code: &str) -> bool {
    let error: &(dyn std::error::Error + 'static) = error;
    std::iter::successors(Some(error), |error| error.source()).any(|error| {
        let message = error.to_string();
        Answer::of_message(&message).is_some_and(|answer| {
            answer.status == Some(status)
                && S3Error::parse(answer.body.as_bytes())
                    .as_ref()
                    .and_then(S3Error::code)
                    == Some(code)
        })
    })
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use super::copy_ranges;

    const MIB: u64 = 1024 * 1024;

    fn part_size(size: u64) -> NonZeroU64 {
        NonZeroU64::new(size).unwrap()
    }

    #[test]
    fn copy_ranges_leave_a_short_last_part() {
        assert_eq!(
            copy_ranges(40 * MIB + 3, part_size(16 * MIB)),
            [0..16 * MIB, 16 * MIB..32 * MIB, 32 * MIB..40 * MIB + 3]
        );
    }

    #[test]
    fn copy_ranges_at_an_exact_multiple_have_no_empty_part() {
        assert_eq!(
            copy_ranges(32 * MIB, part_size(16 * MIB)),
            [0..16 * MIB, 16 * MIB..32 * MIB]
        );
    }

    /// 10,000 parts of 16 MiB hold 156.25 GiB, so one byte past that needs
    /// 17 MiB parts, the smallest whole mebibyte that fits.
    #[test]
    fn copy_ranges_past_the_part_limit_grow_the_part_by_whole_mebibytes() {
        let size = 10_000 * 16 * MIB + 1;
        let ranges = copy_ranges(size, part_size(16 * MIB));

        assert_eq!(ranges.len(), 9_412, "ceil(size / 17 MiB) parts");
        assert!(
            ranges
                .iter()
                .take(ranges.len() - 1)
                .all(|range| range.end - range.start == 17 * MIB),
            "every part but the last is 17 MiB"
        );
        assert_eq!(ranges.first().map(|range| range.start), Some(0));
        assert_eq!(ranges.last().map(|range| range.end), Some(size));
        assert!(
            ranges
                .windows(2)
                .all(|pair| matches!(pair, [first, second] if first.end == second.start)),
            "the ranges are contiguous"
        );
    }
}
