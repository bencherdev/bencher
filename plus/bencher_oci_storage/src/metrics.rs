//! What the registry measures, sent to the server's meter with `otel` and nowhere without it.

use std::time::{Duration, Instant};

use http::StatusCode;
use object_store::client::{HttpError, HttpErrorKind};

pub(crate) use meter::{
    blob_upload, buffer_released, buffer_taken, finalize, time_stream, upload_sweep,
};

use crate::s3_error::{Answer, S3Error};

/// Times one storage call to its answer.
///
/// A call dropped before it answers records nothing, since it has no outcome.
pub(crate) async fn time<T, F>(operation: StorageOperation, call: F) -> object_store::Result<T>
where
    F: Future<Output = object_store::Result<T>>,
{
    let start = Instant::now();
    let result = call.await;
    let outcome = match &result {
        Ok(_) => StorageOutcome::Ok,
        Err(error) => StorageOutcome::of(error),
    };
    record(operation, outcome, start.elapsed());
    result
}

/// Records one storage call that its caller timed and judged.
pub(crate) fn record(operation: StorageOperation, outcome: StorageOutcome, duration: Duration) {
    meter::storage_call(operation, outcome, duration);
}

/// Times an upload request, and records it once: at [`UploadPhaseTimer::finish`], or when dropped.
pub struct UploadPhaseTimer {
    phase: UploadPhase,
    start: Instant,
    outcome: Option<PhaseOutcome>,
}

impl UploadPhaseTimer {
    /// Times an upload request from now, its handler's start.
    pub fn new(phase: UploadPhase) -> Self {
        Self {
            phase,
            start: Instant::now(),
            outcome: None,
        }
    }

    pub fn finish(mut self, outcome: PhaseOutcome) {
        self.outcome = Some(outcome);
    }
}

impl Drop for UploadPhaseTimer {
    /// The server runs each handler to its end, so a phase drops unfinished only
    /// when its handler panics (`error`) or the runtime shuts down (`aborted`).
    fn drop(&mut self) {
        let outcome = self.outcome.unwrap_or(if std::thread::panicking() {
            PhaseOutcome::Error
        } else {
            PhaseOutcome::Aborted
        });
        meter::upload_phase(self.phase, outcome, self.start.elapsed());
    }
}

/// Times a blob GET body, and records the pull once, when the body is dropped.
pub struct PullTimer {
    start: Instant,
    size: u64,
    bytes: u64,
}

impl PullTimer {
    /// Times a blob GET body of `size` bytes from `start`, its handler's start.
    pub fn new(start: Instant, size: u64) -> Self {
        Self {
            start,
            size,
            bytes: 0,
        }
    }

    /// Counts `bytes` more sent.
    pub fn sent(&mut self, bytes: usize) {
        self.bytes += bytes as u64;
    }
}

impl Drop for PullTimer {
    /// A body that sent every byte of its blob completed, since the server drops
    /// a body of known length after its last byte, without polling for its end.
    fn drop(&mut self) {
        let outcome = if self.bytes == self.size {
            PullOutcome::Complete
        } else {
            PullOutcome::Aborted
        };
        meter::pull(outcome, self.bytes, self.start.elapsed());
    }
}

/// What a storage call does, which is not always the trait method it arrived through.
#[derive(Debug, Clone, Copy)]
pub(crate) enum StorageOperation {
    Put,
    CreateMultipart,
    PutPart,
    Complete,
    Abort,
    Copy,
    /// One attempt at an `UploadPartCopy`
    PartCopy,
    Rename,
    Get,
    Head,
    Delete,
    List,
}

/// How a storage call answered.
#[derive(Debug, Clone, Copy)]
pub(crate) enum StorageOutcome {
    Ok,
    /// The object is missing, the normal answer to a probe.
    NotFound,
    /// A condition on the call did not hold, as when a create-only put loses.
    Precondition,
    Error(
        #[cfg_attr(
            not(feature = "otel"),
            expect(dead_code, reason = "only the server's meter reads a code")
        )]
        StorageErrorCode,
    ),
}

impl StorageOutcome {
    fn of(error: &object_store::Error) -> Self {
        if matches!(error, object_store::Error::NotFound { .. }) {
            Self::NotFound
        } else if matches!(
            error,
            object_store::Error::Precondition { .. }
                | object_store::Error::AlreadyExists { .. }
                | object_store::Error::NotModified { .. }
        ) {
            Self::Precondition
        } else {
            Self::Error(StorageErrorCode::of(error))
        }
    }
}

/// Why a storage call failed, from a small fixed set that never carries the error's text.
#[derive(Debug, Clone, Copy)]
pub(crate) enum StorageErrorCode {
    /// HTTP 404 on a call that does not report a missing object as such, like a list.
    NotFound,
    /// HTTP 416
    RangeNotSatisfiable,
    /// HTTP 429
    TooManyRequests,
    /// Any other HTTP 4xx
    ClientError,
    /// Any HTTP 5xx
    ServerError,
    /// The store did not answer in time.
    Timeout,
    /// The store could not be reached.
    Connect,
    SignatureDoesNotMatch,
    ExpiredRequest,
    AccessDenied,
    NoSuchKey,
    NoSuchUpload,
    InvalidRange,
    SlowDown,
    InternalError,
    Other,
}

impl StorageErrorCode {
    /// Walks the error's sources for the failing answer or transport failure beneath it.
    fn of(error: &object_store::Error) -> Self {
        let error: &(dyn std::error::Error + 'static) = error;
        std::iter::successors(Some(error), |error| error.source())
            .find_map(Self::of_source)
            .unwrap_or(Self::Other)
    }

    fn of_source(error: &(dyn std::error::Error + 'static)) -> Option<Self> {
        if let Some(error) = error.downcast_ref::<HttpError>() {
            return Self::of_transport(error.kind());
        }
        let message = error.to_string();
        let answer = Answer::of_message(&message)?;
        Some(Self::of_answer(
            answer.status,
            S3Error::parse(answer.body.as_bytes()).as_ref(),
        ))
    }

    /// A failure to reach the store, or `None` for one that names no cause.
    pub(crate) fn of_transport(kind: HttpErrorKind) -> Option<Self> {
        if kind == HttpErrorKind::Timeout {
            Some(Self::Timeout)
        } else if kind == HttpErrorKind::Connect {
            Some(Self::Connect)
        } else {
            None
        }
    }

    /// A failing answer, by the S3 `Code` in its body when that code is a label,
    /// and by its status otherwise.
    pub(crate) fn of_answer(status: Option<StatusCode>, error: Option<&S3Error>) -> Self {
        error
            .and_then(S3Error::code)
            .and_then(Self::of_s3_code)
            .or_else(|| status.map(Self::of_status))
            .unwrap_or(Self::Other)
    }

    /// Only these S3 codes become labels.
    fn of_s3_code(code: &str) -> Option<Self> {
        match code {
            "SignatureDoesNotMatch" => Some(Self::SignatureDoesNotMatch),
            "ExpiredRequest" => Some(Self::ExpiredRequest),
            "AccessDenied" => Some(Self::AccessDenied),
            "NoSuchKey" => Some(Self::NoSuchKey),
            "NoSuchUpload" => Some(Self::NoSuchUpload),
            "InvalidRange" => Some(Self::InvalidRange),
            "SlowDown" => Some(Self::SlowDown),
            "InternalError" => Some(Self::InternalError),
            _ => None,
        }
    }

    fn of_status(status: StatusCode) -> Self {
        match status.as_u16() {
            404 => Self::NotFound,
            416 => Self::RangeNotSatisfiable,
            429 => Self::TooManyRequests,
            400..=499 => Self::ClientError,
            500..=599 => Self::ServerError,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum UploadPhase {
    /// A PATCH that appends to an upload
    Patch,
    /// A PUT that completes an upload
    Finalize,
}

#[derive(Debug, Clone, Copy)]
pub enum PhaseOutcome {
    Ok,
    Error,
    /// The client went away.
    Aborted,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum FinalizeOutcome {
    Ok,
    DigestMismatch,
    /// A session that an earlier request left past its digest check finished.
    Resumed,
    /// A completed object's `ETag` was not the one its parts give.
    EtagMismatch,
    /// Any other failure
    Error,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SweepAction {
    /// A stale session's multipart upload was aborted.
    Aborted,
    /// A stale session's objects were all deleted.
    Deleted,
}

#[derive(Debug, Clone, Copy)]
enum PullOutcome {
    /// The body sent every byte of its blob.
    Complete,
    Aborted,
}

/// The server's meter, which receives each record.
#[cfg(feature = "otel")]
mod meter {
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use std::time::{Duration, Instant};

    use bencher_otel::{ApiCounter, ApiGauge, ApiHistogram, ApiMeter};
    use futures::stream::{BoxStream, Stream, StreamExt as _};

    use super::{
        FinalizeOutcome, PhaseOutcome, PullOutcome, StorageErrorCode, StorageOperation,
        StorageOutcome, SweepAction, UploadPhase,
    };

    pub(super) fn storage_call(
        operation: StorageOperation,
        outcome: StorageOutcome,
        duration: Duration,
    ) {
        ApiMeter::record(
            ApiHistogram::OciStorageRequestDuration(operation.into(), outcome.into()),
            duration.as_secs_f64(),
        );
        if let StorageOutcome::Error(code) = outcome {
            ApiMeter::increment(ApiCounter::OciStorageError(operation.into(), code.into()));
        }
    }

    /// Times one storage call that answers with a stream, to the stream's end or drop.
    pub(crate) fn time_stream<T>(
        operation: StorageOperation,
        stream: BoxStream<'static, object_store::Result<T>>,
    ) -> BoxStream<'static, object_store::Result<T>>
    where
        T: Send + 'static,
    {
        TimedStream {
            stream,
            call: Some(StreamCall {
                operation,
                start: Instant::now(),
                outcome: StorageOutcome::Ok,
            }),
        }
        .boxed()
    }

    pub(super) fn upload_phase(phase: UploadPhase, outcome: PhaseOutcome, duration: Duration) {
        ApiMeter::record(
            ApiHistogram::OciUploadPhaseDuration(phase.into(), outcome.into()),
            duration.as_secs_f64(),
        );
    }

    pub(super) fn pull(outcome: PullOutcome, bytes: u64, duration: Duration) {
        ApiMeter::record_u64(ApiHistogram::OciBlobPullBytes(outcome.into()), bytes);
        ApiMeter::record(
            ApiHistogram::OciBlobPullDuration(outcome.into()),
            duration.as_secs_f64(),
        );
    }

    pub(crate) fn blob_upload(bytes: u64) {
        ApiMeter::record_u64(ApiHistogram::OciBlobUploadBytes, bytes);
    }

    pub(crate) fn finalize(outcome: FinalizeOutcome) {
        ApiMeter::increment(ApiCounter::OciFinalize(outcome.into()));
    }

    pub(crate) fn upload_sweep(action: SweepAction) {
        ApiMeter::increment(ApiCounter::OciUploadSweep(action.into()));
    }

    /// A part buffer was taken after a wait of `waited`.
    pub(crate) fn buffer_taken(waited: Duration) {
        ApiMeter::record(ApiHistogram::OciUploadBufferWait, waited.as_secs_f64());
        ApiMeter::up(ApiGauge::OciUploadBuffers);
    }

    pub(crate) fn buffer_released() {
        ApiMeter::down(ApiGauge::OciUploadBuffers);
    }

    /// A stream from a storage call, which records the call once, at the stream's end or drop.
    struct TimedStream<T> {
        stream: BoxStream<'static, object_store::Result<T>>,
        call: Option<StreamCall>,
    }

    #[deny(clippy::missing_trait_methods)]
    impl<T> Stream for TimedStream<T> {
        type Item = object_store::Result<T>;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let polled = self.stream.poll_next_unpin(cx);
            match &polled {
                Poll::Ready(Some(Err(error))) => {
                    if let Some(call) = &mut self.call
                        && matches!(call.outcome, StorageOutcome::Ok)
                    {
                        call.outcome = StorageOutcome::of(error);
                    }
                },
                Poll::Ready(None) => self.call = None,
                Poll::Ready(Some(Ok(_))) | Poll::Pending => {},
            }
            polled
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            self.stream.size_hint()
        }
    }

    /// A storage call that answered with a stream, recorded with its first failure when dropped.
    struct StreamCall {
        operation: StorageOperation,
        start: Instant,
        outcome: StorageOutcome,
    }

    impl Drop for StreamCall {
        fn drop(&mut self) {
            storage_call(self.operation, self.outcome, self.start.elapsed());
        }
    }

    impl From<StorageOperation> for bencher_otel::OciStorageOperation {
        fn from(operation: StorageOperation) -> Self {
            match operation {
                StorageOperation::Put => Self::Put,
                StorageOperation::CreateMultipart => Self::CreateMultipart,
                StorageOperation::PutPart => Self::PutPart,
                StorageOperation::Complete => Self::Complete,
                StorageOperation::Abort => Self::Abort,
                StorageOperation::Copy => Self::Copy,
                StorageOperation::PartCopy => Self::PartCopy,
                StorageOperation::Rename => Self::Rename,
                StorageOperation::Get => Self::Get,
                StorageOperation::Head => Self::Head,
                StorageOperation::Delete => Self::Delete,
                StorageOperation::List => Self::List,
            }
        }
    }

    impl From<StorageOutcome> for bencher_otel::OciStorageOutcome {
        fn from(outcome: StorageOutcome) -> Self {
            match outcome {
                StorageOutcome::Ok => Self::Ok,
                StorageOutcome::NotFound => Self::NotFound,
                StorageOutcome::Precondition => Self::Precondition,
                StorageOutcome::Error(_) => Self::Error,
            }
        }
    }

    impl From<StorageErrorCode> for bencher_otel::OciStorageErrorCode {
        fn from(code: StorageErrorCode) -> Self {
            match code {
                StorageErrorCode::NotFound => Self::NotFound,
                StorageErrorCode::RangeNotSatisfiable => Self::RangeNotSatisfiable,
                StorageErrorCode::TooManyRequests => Self::TooManyRequests,
                StorageErrorCode::ClientError => Self::ClientError,
                StorageErrorCode::ServerError => Self::ServerError,
                StorageErrorCode::Timeout => Self::Timeout,
                StorageErrorCode::Connect => Self::Connect,
                StorageErrorCode::SignatureDoesNotMatch => Self::SignatureDoesNotMatch,
                StorageErrorCode::ExpiredRequest => Self::ExpiredRequest,
                StorageErrorCode::AccessDenied => Self::AccessDenied,
                StorageErrorCode::NoSuchKey => Self::NoSuchKey,
                StorageErrorCode::NoSuchUpload => Self::NoSuchUpload,
                StorageErrorCode::InvalidRange => Self::InvalidRange,
                StorageErrorCode::SlowDown => Self::SlowDown,
                StorageErrorCode::InternalError => Self::InternalError,
                StorageErrorCode::Other => Self::Other,
            }
        }
    }

    impl From<UploadPhase> for bencher_otel::OciUploadPhase {
        fn from(phase: UploadPhase) -> Self {
            match phase {
                UploadPhase::Patch => Self::Patch,
                UploadPhase::Finalize => Self::Finalize,
            }
        }
    }

    impl From<PhaseOutcome> for bencher_otel::OciPhaseOutcome {
        fn from(outcome: PhaseOutcome) -> Self {
            match outcome {
                PhaseOutcome::Ok => Self::Ok,
                PhaseOutcome::Error => Self::Error,
                PhaseOutcome::Aborted => Self::Aborted,
            }
        }
    }

    impl From<FinalizeOutcome> for bencher_otel::OciFinalizeOutcome {
        fn from(outcome: FinalizeOutcome) -> Self {
            match outcome {
                FinalizeOutcome::Ok => Self::Ok,
                FinalizeOutcome::DigestMismatch => Self::DigestMismatch,
                FinalizeOutcome::Resumed => Self::Resumed,
                FinalizeOutcome::EtagMismatch => Self::EtagMismatch,
                FinalizeOutcome::Error => Self::Error,
            }
        }
    }

    impl From<SweepAction> for bencher_otel::OciSweepAction {
        fn from(action: SweepAction) -> Self {
            match action {
                SweepAction::Aborted => Self::Aborted,
                SweepAction::Deleted => Self::Deleted,
            }
        }
    }

    impl From<PullOutcome> for bencher_otel::OciPullOutcome {
        fn from(outcome: PullOutcome) -> Self {
            match outcome {
                PullOutcome::Complete => Self::Complete,
                PullOutcome::Aborted => Self::Aborted,
            }
        }
    }
}

/// Without `otel`, nothing records.
#[cfg(not(feature = "otel"))]
mod meter {
    use std::time::Duration;

    use futures::stream::BoxStream;

    use super::{
        FinalizeOutcome, PhaseOutcome, PullOutcome, StorageOperation, StorageOutcome, SweepAction,
        UploadPhase,
    };

    pub(super) fn storage_call(
        _operation: StorageOperation,
        _outcome: StorageOutcome,
        _duration: Duration,
    ) {
    }

    pub(crate) fn time_stream<T>(
        _operation: StorageOperation,
        stream: BoxStream<'static, object_store::Result<T>>,
    ) -> BoxStream<'static, object_store::Result<T>> {
        stream
    }

    pub(super) fn upload_phase(_phase: UploadPhase, _outcome: PhaseOutcome, _duration: Duration) {}

    pub(super) fn pull(_outcome: PullOutcome, _bytes: u64, _duration: Duration) {}

    pub(crate) fn blob_upload(_bytes: u64) {}

    pub(crate) fn finalize(_outcome: FinalizeOutcome) {}

    pub(crate) fn upload_sweep(_action: SweepAction) {}

    pub(crate) fn buffer_taken(_waited: Duration) {}

    pub(crate) fn buffer_released() {}
}
