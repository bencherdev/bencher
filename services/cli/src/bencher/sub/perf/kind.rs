use bencher_json::perf::JsonPerfKind;

use crate::cli::perf::CliPerfKind;

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Latency,
    Throughput,
    Compute,
    Memory,
    Storage,
}

impl From<CliPerfKind> for Kind {
    fn from(kind: CliPerfKind) -> Self {
        match kind {
            CliPerfKind::Latency => Self::Latency,
            CliPerfKind::Throughput => Self::Throughput,
            CliPerfKind::Compute => Self::Compute,
            CliPerfKind::Memory => Self::Memory,
            CliPerfKind::Storage => Self::Storage,
        }
    }
}

impl Into<JsonPerfKind> for Kind {
    fn into(self) -> JsonPerfKind {
        match self {
            Self::Latency => JsonPerfKind::Latency,
            Self::Throughput => JsonPerfKind::Throughput,
            Self::Compute => JsonPerfKind::Compute,
            Self::Memory => JsonPerfKind::Memory,
            Self::Storage => JsonPerfKind::Storage,
        }
    }
}
