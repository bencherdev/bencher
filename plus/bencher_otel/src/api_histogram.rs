use bencher_callback::CallbackFinish;
use opentelemetry::KeyValue;

pub use bencher_json::Priority;

use crate::callback::finish_attributes;
use crate::oci::{
    BYTES_BOUNDARIES, OciPhaseOutcome, OciPullOutcome, OciStorageOperation, OciStorageOutcome,
    OciUploadPhase, SECONDS_BOUNDARIES,
};

#[derive(Debug, Clone, Copy)]
pub enum ApiHistogram {
    /// Time a job spent waiting in the queue before being claimed.
    JobQueueDuration(Priority),
    /// Actual execution time from job started to completion (excludes queue wait).
    JobRunDuration(Priority),
    /// Total time from job creation to completion.
    JobCompleteDuration(Priority),
    /// Total wall-clock time for the entire report creation endpoint.
    ReportCreateDuration,
    /// Total time to process report results (adapter parsing + all iterations).
    ReportProcessDuration,
    /// Time spent in the batched DB write transaction per iteration.
    ReportWriteDuration,
    /// Time from a callback's claim to its final outcome.
    CallbackFinishDuration(CallbackFinish),
    /// Attempts a callback made over its life, recorded at its final outcome.
    CallbackFinishAttempts(CallbackFinish),
    /// Time a registry storage call took to answer.
    OciStorageRequestDuration(OciStorageOperation, OciStorageOutcome),
    /// Time an upload request took, from its handler's start to its response.
    OciUploadPhaseDuration(OciUploadPhase, OciPhaseOutcome),
    /// Size of each finalized blob.
    OciBlobUploadBytes,
    /// Bytes a blob GET body sent before it ended or was dropped.
    OciBlobPullBytes(OciPullOutcome),
    /// Time a blob GET took, from its handler's start to its body's end or drop.
    OciBlobPullDuration(OciPullOutcome),
    /// Time an upload waited for a part buffer.
    OciUploadBufferWait,
}

impl ApiHistogram {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::JobQueueDuration(_) => "job.queue.duration",
            Self::JobRunDuration(_) => "job.run.duration",
            Self::JobCompleteDuration(_) => "job.complete.duration",
            Self::ReportCreateDuration => "report.create.duration",
            Self::ReportProcessDuration => "report.process.duration",
            Self::ReportWriteDuration => "report.write.duration",
            Self::CallbackFinishDuration(_) => "callback.finish.duration",
            Self::CallbackFinishAttempts(_) => "callback.finish.attempts",
            Self::OciStorageRequestDuration(_, _) => "oci.storage.request.duration",
            Self::OciUploadPhaseDuration(_, _) => "oci.upload.phase.duration",
            Self::OciBlobUploadBytes => "oci.blob.upload.bytes",
            Self::OciBlobPullBytes(_) => "oci.blob.pull.bytes",
            Self::OciBlobPullDuration(_) => "oci.blob.pull.duration",
            Self::OciUploadBufferWait => "oci.upload.buffer.wait",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::JobQueueDuration(_) => {
                "Time a job spent waiting in the queue before being claimed"
            },
            Self::JobRunDuration(_) => "Actual execution time from job started to completion",
            Self::JobCompleteDuration(_) => "Total time from job creation to completion",
            Self::ReportCreateDuration => {
                "Total wall-clock time for the entire report creation endpoint"
            },
            Self::ReportProcessDuration => {
                "Total time to process report results (adapter parsing + all iterations)"
            },
            Self::ReportWriteDuration => {
                "Time spent in the batched DB write transaction per iteration"
            },
            Self::CallbackFinishDuration(_) => "Time from a callback's claim to its final outcome",
            Self::CallbackFinishAttempts(_) => {
                "Attempts a callback made over its life, recorded at its final outcome"
            },
            Self::OciStorageRequestDuration(_, _) => "Time a registry storage call took to answer",
            Self::OciUploadPhaseDuration(_, _) => {
                "Time an upload request took, from its handler's start to its response"
            },
            Self::OciBlobUploadBytes => "Size of each finalized blob",
            Self::OciBlobPullBytes(_) => {
                "Bytes a blob GET body sent before it ended or was dropped"
            },
            Self::OciBlobPullDuration(_) => {
                "Time a blob GET took, from its handler's start to its body's end or drop"
            },
            Self::OciUploadBufferWait => "Time an upload waited for a part buffer",
        }
    }

    pub(crate) fn unit(self) -> &'static str {
        match self {
            Self::JobQueueDuration(_)
            | Self::JobRunDuration(_)
            | Self::JobCompleteDuration(_)
            | Self::ReportCreateDuration
            | Self::ReportProcessDuration
            | Self::ReportWriteDuration
            | Self::CallbackFinishDuration(_)
            | Self::OciStorageRequestDuration(_, _)
            | Self::OciUploadPhaseDuration(_, _)
            | Self::OciBlobPullDuration(_)
            | Self::OciUploadBufferWait => "s",
            Self::CallbackFinishAttempts(_) => "{attempt}",
            Self::OciBlobUploadBytes | Self::OciBlobPullBytes(_) => "By",
        }
    }

    pub(crate) fn attributes(self) -> Vec<KeyValue> {
        match self {
            Self::JobQueueDuration(priority)
            | Self::JobRunDuration(priority)
            | Self::JobCompleteDuration(priority) => vec![priority_attribute(priority)],
            Self::ReportCreateDuration
            | Self::ReportProcessDuration
            | Self::ReportWriteDuration
            | Self::OciBlobUploadBytes
            | Self::OciUploadBufferWait => Vec::new(),
            Self::CallbackFinishDuration(finish) | Self::CallbackFinishAttempts(finish) => {
                finish_attributes(finish)
            },
            Self::OciStorageRequestDuration(operation, outcome) => {
                vec![operation.into(), outcome.into()]
            },
            Self::OciUploadPhaseDuration(phase, outcome) => vec![phase.into(), outcome.into()],
            Self::OciBlobPullBytes(outcome) | Self::OciBlobPullDuration(outcome) => {
                vec![outcome.into()]
            },
        }
    }

    /// Boundaries for values that the SDK's defaults, sized for milliseconds, would lump into one or two buckets.
    pub(crate) fn boundaries(self) -> Option<&'static [f64]> {
        match self {
            Self::JobQueueDuration(_)
            | Self::JobRunDuration(_)
            | Self::JobCompleteDuration(_)
            | Self::ReportCreateDuration
            | Self::ReportProcessDuration
            | Self::ReportWriteDuration => None,
            // Three attempts of up to 10 s, 4 s and 16 s apart.
            Self::CallbackFinishDuration(_) => {
                Some(&[0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 20.0, 30.0, 60.0])
            },
            Self::CallbackFinishAttempts(_) => Some(&[0.0, 1.0, 2.0, 3.0]),
            Self::OciStorageRequestDuration(_, _)
            | Self::OciUploadPhaseDuration(_, _)
            | Self::OciBlobPullDuration(_)
            | Self::OciUploadBufferWait => Some(SECONDS_BOUNDARIES),
            Self::OciBlobUploadBytes | Self::OciBlobPullBytes(_) => Some(BYTES_BOUNDARIES),
        }
    }
}

pub(crate) fn priority_attribute(priority: Priority) -> KeyValue {
    KeyValue::new("job.priority", priority.to_string())
}
