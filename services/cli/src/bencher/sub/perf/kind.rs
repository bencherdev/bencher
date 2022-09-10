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

impl From<Kind> for JsonPerfKind {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Latency => Self::Latency,
            Kind::Throughput => Self::Throughput,
            Kind::Compute => Self::Compute,
            Kind::Memory => Self::Memory,
            Kind::Storage => Self::Storage,
        }
    }
}
