use core::fmt;

#[derive(Debug, Clone, Copy)]
pub enum ApiGauge {
    RunnerState(RunnerStateKind),
    CallbackPending,
    OciUploadBuffers,
}

impl ApiGauge {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::RunnerState(_) => "runner.state",
            Self::CallbackPending => "callback.pending",
            Self::OciUploadBuffers => "oci.upload.buffers",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::RunnerState(_) => "Current number of runners in each state",
            Self::CallbackPending => "Current number of callbacks waiting to be delivered",
            Self::OciUploadBuffers => "Current number of upload part buffers in use",
        }
    }

    pub(crate) fn unit(self) -> &'static str {
        match self {
            Self::RunnerState(_) => "{runner}",
            Self::CallbackPending => "{callback}",
            Self::OciUploadBuffers => "{buffer}",
        }
    }

    pub(crate) fn attributes(self) -> Vec<opentelemetry::KeyValue> {
        match self {
            Self::RunnerState(state) => vec![state.into()],
            Self::CallbackPending | Self::OciUploadBuffers => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RunnerStateKind {
    Idle,
    Paused,
    Executing,
    Updating,
}

impl fmt::Display for RunnerStateKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Paused => write!(f, "paused"),
            Self::Executing => write!(f, "executing"),
            Self::Updating => write!(f, "updating"),
        }
    }
}

impl From<RunnerStateKind> for opentelemetry::KeyValue {
    fn from(state: RunnerStateKind) -> Self {
        Self::new(RunnerStateKind::KEY, state.to_string())
    }
}

impl RunnerStateKind {
    const KEY: &str = "runner.state";
}
