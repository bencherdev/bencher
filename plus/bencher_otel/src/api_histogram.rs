use opentelemetry::KeyValue;

pub use bencher_json::Priority;

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
        }
    }

    pub(crate) fn unit(self) -> &'static str {
        match self {
            Self::JobQueueDuration(_)
            | Self::JobRunDuration(_)
            | Self::JobCompleteDuration(_)
            | Self::ReportCreateDuration
            | Self::ReportProcessDuration
            | Self::ReportWriteDuration => "s",
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
        }
    }
}

pub(crate) fn priority_attribute(priority: Priority) -> KeyValue {
    KeyValue::new("job.priority", priority.to_string())
}
