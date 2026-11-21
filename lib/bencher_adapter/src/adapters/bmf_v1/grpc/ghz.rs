use bencher_json::{
    BenchmarkName, MeasureNameId, ParameterSet,
    project::measure::built_in::{
        BuiltInMeasure as _,
        default::{Latency, Throughput},
        grpc::{
            StatusAborted, StatusAlreadyExists, StatusCancelled, StatusDataLoss,
            StatusDeadlineExceeded, StatusFailedPrecondition, StatusInternal,
            StatusInvalidArgument, StatusNotFound, StatusOk, StatusOutOfRange,
            StatusPermissionDenied, StatusResourceExhausted, StatusUnauthenticated,
            StatusUnavailable, StatusUnimplemented, StatusUnknown,
        },
        http::{FailedRequests, Requests},
    },
};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        grpc::{ghz_json::AdapterGrpcGhzJson, ghz_text::AdapterGrpcGhzText},
        leaf::{LeafResults, Variant},
    },
    results::adapter_results::AdapterResults,
};

/// ghz at version 1: its JSON report, then its text summary.
pub struct AdapterGrpcGhz;

impl Adaptable for AdapterGrpcGhz {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterGrpcGhzJson::parse(input, settings)
            .or_else(|| AdapterGrpcGhzText::parse(input, settings))
    }
}

/// What one ghz run reports, as either leaf reads it, times in nanoseconds.
pub(super) struct Run {
    /// The run's benchmark, or `None` for a text summary with no `Name:`, which the server names
    /// by the run's benchmark prefix, else `ghz`.
    pub benchmark: Option<BenchmarkName>,
    pub parameters: ParameterSet,
    /// Every call counted, errors included.
    pub count: f64,
    /// The mean time of every call counted, errors included (`runner/reporter.go:253-255` at
    /// v0.121.0).
    pub average: f64,
    /// The latency distribution, `None` when ghz counted no call in it.
    pub distribution: Option<Distribution>,
    pub rps: f64,
    /// How many calls ended with each status, by the name Go's gRPC prints for it.
    pub statuses: Vec<(String, f64)>,
}

/// The calls ghz counts in its latency distribution: those that ended OK, or every call with
/// `--count-errors` (`runner/reporter.go:259-275` at v0.121.0).
pub(super) struct Distribution {
    pub fastest: f64,
    pub slowest: f64,
    /// Each percentile by its metric name.
    pub percentiles: Vec<(String, f64)>,
}

impl Run {
    pub(super) fn insert(self, results: &mut LeafResults) {
        let Self {
            benchmark,
            parameters,
            count,
            average,
            distribution,
            rps,
            statuses,
        } = self;
        let mut variant = if let Some(benchmark) = benchmark {
            results.variant(benchmark, parameters)
        } else {
            results.nameless_variant(GHZ, parameters)
        };
        let latency = Latency::name_id();
        // With no call counted, ghz leaves its `average` at 0.
        if count > 0.0 {
            variant.insert(latency.clone(), "mean", average);
        }
        if let Some(Distribution {
            fastest,
            slowest,
            percentiles,
        }) = distribution
        {
            variant.insert(latency.clone(), "min", fastest);
            variant.insert(latency.clone(), "max", slowest);
            for (metric, value) in percentiles {
                variant.insert(latency.clone(), &metric, value);
            }
        }
        variant.insert(Throughput::name_id(), "mean", rps);
        variant.insert(Requests::name_id(), VALUE, count);
        let mut failed = 0.0;
        for (status, calls) in statuses {
            if status != OK {
                failed += calls;
            }
            if let Some(measure) = status_measure(&mut variant, &status) {
                variant.insert(measure, VALUE, calls);
            }
        }
        variant.insert(FailedRequests::name_id(), VALUE, failed);
    }
}

/// The name of ghz's benchmark when the output names none and the run sets no prefix.
const GHZ: &str = "ghz";
const VALUE: &str = "value";
const OK: &str = "OK";

/// The measure of the calls that ended with `status`: built in for a code gRPC registers, else
/// `grpc-status-<code>` named at run time for the `Code(<code>)` Go prints for any other code
/// (`codes/code_string.go:31-70` at grpc-go v1.56.3), or `None`, counted as skipped, for the
/// empty status ghz records for an error that carries none (`runner/stats_handler.go:46-50`).
fn status_measure(variant: &mut Variant<'_>, status: &str) -> Option<MeasureNameId> {
    if let Some(measure) = registered(status) {
        return Some(measure);
    }
    if let Some(code) = status
        .strip_prefix("Code(")
        .and_then(|code| code.strip_suffix(')'))
        .filter(|code| !code.is_empty() && code.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return variant.named(&format!("grpc-status-{code}"), StatusOk::UNITS_STR);
    }
    variant.skip_measure();
    None
}

/// The built-in measure of a status code gRPC registers, by the name Go prints for it.
fn registered(status: &str) -> Option<MeasureNameId> {
    Some(match status {
        OK => StatusOk::name_id(),
        "Canceled" => StatusCancelled::name_id(),
        "Unknown" => StatusUnknown::name_id(),
        "InvalidArgument" => StatusInvalidArgument::name_id(),
        "DeadlineExceeded" => StatusDeadlineExceeded::name_id(),
        "NotFound" => StatusNotFound::name_id(),
        "AlreadyExists" => StatusAlreadyExists::name_id(),
        "PermissionDenied" => StatusPermissionDenied::name_id(),
        "ResourceExhausted" => StatusResourceExhausted::name_id(),
        "FailedPrecondition" => StatusFailedPrecondition::name_id(),
        "Aborted" => StatusAborted::name_id(),
        "OutOfRange" => StatusOutOfRange::name_id(),
        "Unimplemented" => StatusUnimplemented::name_id(),
        "Internal" => StatusInternal::name_id(),
        "Unavailable" => StatusUnavailable::name_id(),
        "DataLoss" => StatusDataLoss::name_id(),
        "Unauthenticated" => StatusUnauthenticated::name_id(),
        _ => return None,
    })
}

#[cfg(test)]
mod test_grpc_ghz {
    use bencher_json::project::report::Adapter;
    use pretty_assertions::assert_eq;

    use crate::adapters::bmf_v1::test_util::convert_v1;

    /// Kills a node that leaves out either leaf.
    #[test]
    fn adapter_grpc_ghz_reads_both_leaves() {
        for (suffix, leaf) in [
            ("grpc/ghz/ok.json", Adapter::GrpcGhzJson),
            ("grpc/ghz/ok_summary.txt", Adapter::GrpcGhzText),
        ] {
            let results = convert_v1(Adapter::GrpcGhz, suffix);
            assert_eq!(results.resolved_adapter, Some(leaf), "{suffix}");
            assert_eq!(results, convert_v1(leaf, suffix), "{suffix}");
        }
    }
}
