use bencher_callback::CallbackFinish;
use opentelemetry::KeyValue;

pub use bencher_json::Priority;

use crate::callback::finish_attributes;

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
            | Self::CallbackFinishDuration(_) => "s",
            Self::CallbackFinishAttempts(_) => "{attempt}",
        }
    }

    pub(crate) fn attributes(self) -> Vec<KeyValue> {
        match self {
            Self::JobQueueDuration(priority)
            | Self::JobRunDuration(priority)
            | Self::JobCompleteDuration(priority) => vec![priority_attribute(priority)],
            Self::ReportCreateDuration
            | Self::ReportProcessDuration
            | Self::ReportWriteDuration => Vec::new(),
            Self::CallbackFinishDuration(finish) | Self::CallbackFinishAttempts(finish) => {
                finish_attributes(finish)
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
        }
    }
}

pub(crate) fn priority_attribute(priority: Priority) -> KeyValue {
    KeyValue::new("job.priority", priority.to_string())
}
