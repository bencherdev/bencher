use core::fmt;

#[derive(Debug, Clone, Copy)]
pub enum ApiGauge {
    RunnerState(RunnerStateKind),
}

impl ApiGauge {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::RunnerState(_) => "runner.state",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::RunnerState(_) => "Current number of runners in each state",
        }
    }

    pub(crate) fn unit(self) -> &'static str {
        match self {
            Self::RunnerState(_) => "{runner}",
        }
    }

    pub(crate) fn attributes(self) -> Vec<opentelemetry::KeyValue> {
        match self {
            Self::RunnerState(state) => vec![state.into()],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RunnerStateKind {
    Idle,
    Executing,
    Updating,
}

impl fmt::Display for RunnerStateKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Executing => write!(f, "executing"),
            Self::Updating => write!(f, "updating"),
        }
    }
}

impl From<RunnerStateKind> for opentelemetry::KeyValue {
    fn from(state: RunnerStateKind) -> Self {
        opentelemetry::KeyValue::new(RunnerStateKind::KEY, state.to_string())
    }
}

impl RunnerStateKind {
    const KEY: &str = "runner.state";
}
