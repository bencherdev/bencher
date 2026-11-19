use std::collections::BTreeMap;

use bencher_json::{
    BenchmarkName, MeasureNameId,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Latency, Throughput},
            http::{
                ConnectingTime, ConnectionsOpened, FailedRequests, Requests, Responses,
                Responses1xx, Responses2xx, Responses3xx, Responses4xx, Responses5xx,
                SizePerRequest,
            },
        },
        report::Adapter,
    },
};
use serde::Deserialize;
use serde_json::{Map, Number, Value};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        finite::Finite,
        http::k6::scale,
        leaf::{LeafResults, Variant, parameter_set},
        percentile::percentile_name,
    },
    results::adapter_results::AdapterResults,
};

/// Fortio at version 1: its `-json` result of an HTTP, TCP, UDP, gRPC ping, or gRPC health run.
///
/// One benchmark, named by its labels, else by its target, with its run type and requested
/// settings as parameters.
pub struct AdapterHttpFortio;

impl Adaptable for AdapterHttpFortio {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let report = serde_json::from_str::<Report>(input).ok()?;
        if report.aborted {
            return None;
        }
        let run_type = report.target.run_type(&report.run_type)?;
        let mut results = LeafResults::default();
        report.insert(&run_type, &mut results);
        results.finish_with_skips(Adapter::HttpFortio)
    }
}

/// The keys the leaf reads of every run (`periodic/periodic.go:242-266` at 1.76.0), and of its
/// target.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Report {
    run_type: String,
    labels: String,
    #[serde(rename = "RequestedQPS")]
    requested_qps: String,
    requested_duration: String,
    #[serde(rename = "ActualQPS")]
    actual_qps: Finite,
    num_threads: Number,
    duration_histogram: Histogram,
    errors_duration_histogram: Histogram,
    exactly: i64,
    jitter: Switch,
    uniform: Switch,
    no_catch_up: Switch,
    ret_codes: BTreeMap<String, Finite>,
    /// Set on a run that aborted before it started, whose histograms hold nothing.
    #[serde(default, rename = "error")]
    aborted: bool,
    #[serde(flatten)]
    target: Target,
}

/// What the run targets, told apart by the keys its runner writes.
#[derive(Deserialize)]
#[serde(untagged)]
enum Target {
    Http(Box<Http>),
    Echo(Echo),
    Grpc(Grpc),
}

/// `fhttp/httprunner.go:39-58` and `fhttp/http_client.go:185-227`.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Http {
    #[serde(rename = "URL")]
    url: String,
    #[serde(rename = "HTTPReqTimeOut")]
    timeout: Number,
    h2: Switch,
    #[serde(rename = "HTTP10")]
    http10: Switch,
    disable_fast_client: Switch,
    disable_keep_alive: Switch,
    compression: Switch,
    sizes: Histogram,
    connection_stats: Histogram,
    socket_count: Finite,
}

/// A TCP or UDP echo run (`tcprunner/tcprunner.go:38-69`, `udprunner/udprunner.go:42-72`), whose
/// `ReqTimeout` is always 0 (`tcprunner/tcprunner.go:245-249`, `udprunner/udprunner.go:233-237`).
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Echo {
    destination: String,
    socket_count: Finite,
}

/// `fgrpc/grpcrunner.go:76-88`.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Grpc {
    destination: String,
}

/// `stats/stats.go:192-201`.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Histogram {
    count: Finite,
    min: Finite,
    max: Finite,
    avg: Finite,
    std_dev: Finite,
    /// Absent from `Sizes` and from a histogram with no samples.
    #[serde(default)]
    percentiles: Vec<Percentile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Percentile {
    percentile: f64,
    value: Finite,
}

/// A setting Fortio echoes as `true` or `false`.
#[derive(Clone, Copy, Deserialize)]
#[serde(from = "bool")]
enum Switch {
    On,
    Off,
}

const VALUE: &str = "value";
const RESPONSES: &str = "@responses";
const DIRECTION: &str = "@direction";
const NANOS_PER_SECOND: u32 = 1_000_000_000;
/// The code of every HTTP socket error (`fhttp/http_client.go:963`).
const SOCKET_ERROR: &str = "-1";

impl Report {
    fn insert(self, run_type: &RunType, results: &mut LeafResults) {
        let Ok(name) = self.name().parse::<BenchmarkName>() else {
            results.skip_benchmark();
            return;
        };
        let parameters = self.parameters(run_type);
        if let Some(mut run) = variant(results, &name, parameters.clone()) {
            self.duration_histogram
                .insert(&mut run, &Latency::name_id(), NANOS_PER_SECOND);
            self.actual_qps
                .insert(&mut run, &Throughput::name_id(), "mean", 1.0);
            self.duration_histogram
                .count
                .insert(&mut run, &Requests::name_id(), VALUE, 1.0);
            match &self.target {
                Target::Http(http) => {
                    http.connection_stats.insert(
                        &mut run,
                        &ConnectingTime::name_id(),
                        NANOS_PER_SECOND,
                    );
                    http.socket_count
                        .insert(&mut run, &ConnectionsOpened::name_id(), VALUE, 1.0);
                    insert_status_codes(&mut run, &self.ret_codes);
                },
                Target::Echo(echo) => {
                    echo.socket_count
                        .insert(&mut run, &ConnectionsOpened::name_id(), VALUE, 1.0);
                    insert_failures(&mut run, &self.ret_codes);
                },
                Target::Grpc(_) => insert_failures(&mut run, &self.ret_codes),
            }
        }
        if self.errors_duration_histogram.has_samples() {
            let mut parameters = parameters.clone();
            parameters.insert(RESPONSES.to_owned(), "error".into());
            if let Some(mut errors) = variant(results, &name, parameters) {
                self.errors_duration_histogram.insert(
                    &mut errors,
                    &Latency::name_id(),
                    NANOS_PER_SECOND,
                );
            }
        }
        if let Target::Http(http) = &self.target
            && http.sizes.has_samples()
        {
            let mut parameters = parameters;
            parameters.insert(DIRECTION.to_owned(), "received".into());
            if let Some(mut received) = variant(results, &name, parameters) {
                http.sizes
                    .insert(&mut received, &SizePerRequest::name_id(), 1);
            }
        }
    }

    /// The user's labels, or the target when they are Fortio's own default: the target without
    /// `https://` or `http://`, then ` , ` and the host's name up to its first `.`
    /// (`cli/fortio_main.go:398-408`), which would start a new series on every host.
    fn name(&self) -> &str {
        let target = self.target.address();
        let short = ["https://", "http://"]
            .into_iter()
            .find_map(|scheme| target.strip_prefix(scheme))
            .unwrap_or(target);
        let default = self
            .labels
            .strip_prefix(short)
            .and_then(|host| host.strip_prefix(" , "))
            .is_some_and(|host| !host.contains('.'));
        if default { target } else { &self.labels }
    }

    /// The run type, and the requested settings as Fortio's flags name them.
    fn parameters(&self, run_type: &RunType) -> Map<String, Value> {
        let mut parameters = Map::from_iter([
            ("@runType".to_owned(), run_type.as_str().into()),
            ("@qps".to_owned(), self.requested_qps.clone().into()),
            ("@threads".to_owned(), self.num_threads.clone().into()),
            ("@uniform".to_owned(), self.uniform.is_on().into()),
            ("@jitter".to_owned(), self.jitter.is_on().into()),
            ("@nocatchup".to_owned(), self.no_catch_up.is_on().into()),
        ]);
        if self.exactly > 0 {
            parameters.insert("@exactly".to_owned(), self.exactly.into());
        } else {
            parameters.insert(
                "@duration".to_owned(),
                self.requested_duration.clone().into(),
            );
        }
        if let Target::Http(http) = &self.target {
            for (key, value) in [
                ("@timeout", http.timeout.clone().into()),
                ("@h2", http.h2.is_on().into()),
                ("@http1.0", http.http10.is_on().into()),
                ("@stdclient", http.disable_fast_client.is_on().into()),
                ("@keepalive", (!http.disable_keep_alive.is_on()).into()),
                ("@compression", http.compression.is_on().into()),
            ] {
                parameters.insert(key.to_owned(), value);
            }
        }
        if let RunType::GrpcHealth(service) = run_type {
            parameters.insert("@service".to_owned(), service.as_str().into());
        }
        parameters
    }
}

impl Target {
    /// The run Fortio names `run_type` against this target, if the leaf reads it. A custom gRPC
    /// method's run counts each failed call twice (`fgrpc/grpcrunner.go:104-122`), so it is not
    /// read.
    fn run_type(&self, run_type: &str) -> Option<RunType> {
        match self {
            Self::Http(_) => (run_type == "HTTP").then_some(RunType::Http),
            Self::Echo(_) => match run_type {
                "TCP" => Some(RunType::Tcp),
                "UDP" => Some(RunType::Udp),
                _ => None,
            },
            Self::Grpc(_) => {
                if run_type.starts_with("GRPC Ping") {
                    Some(RunType::GrpcPing)
                } else {
                    RunType::health(run_type)
                }
            },
        }
    }

    fn address(&self) -> &str {
        match self {
            Self::Http(http) => &http.url,
            Self::Echo(Echo { destination, .. }) | Self::Grpc(Grpc { destination }) => destination,
        }
    }
}

/// The kind of run, written as `@runType`.
enum RunType {
    Http,
    Tcp,
    Udp,
    GrpcPing,
    /// A gRPC health check of the service it names, empty for the server's overall health.
    GrpcHealth(String),
}

impl RunType {
    /// `GRPC Health for '<service>'`, then ` PayloadLength=<n>` when the run sends a payload
    /// (`fgrpc/grpcrunner.go:171-177` at 1.76.0), so only a run with no payload ends in `'`.
    fn health(run_type: &str) -> Option<Self> {
        let quoted = run_type.strip_prefix("GRPC Health for '")?;
        let service = quoted.strip_suffix('\'').or_else(|| {
            let (quoted, _) = quoted.rsplit_once(" PayloadLength=")?;
            quoted.strip_suffix('\'')
        })?;
        Some(Self::GrpcHealth(service.to_owned()))
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Tcp => "tcp",
            Self::Udp => "udp",
            Self::GrpcPing => "grpc_ping",
            Self::GrpcHealth(_) => "grpc_health",
        }
    }
}

impl Switch {
    fn is_on(self) -> bool {
        matches!(self, Self::On)
    }
}

impl From<bool> for Switch {
    fn from(on: bool) -> Self {
        if on { Self::On } else { Self::Off }
    }
}

/// The benchmark's variant of `parameters`, or `None`, counted as skipped, past the key cap.
fn variant<'a>(
    results: &'a mut LeafResults,
    name: &BenchmarkName,
    parameters: Map<String, Value>,
) -> Option<Variant<'a>> {
    let Some(parameters) = parameter_set(parameters) else {
        results.skip_variant();
        return None;
    };
    Some(results.variant(name.clone(), parameters))
}

impl Histogram {
    /// An empty histogram prints every statistic as 0.
    fn has_samples(&self) -> bool {
        self.count.get().is_some_and(|count| count > 0.0)
    }

    /// Each statistic as `measure`'s metric, times `factor`, when the histogram holds a sample.
    fn insert(&self, variant: &mut Variant<'_>, measure: &MeasureNameId, factor: u32) {
        if !self.has_samples() {
            return;
        }
        for (metric, value) in [
            ("mean", self.avg),
            ("std_dev", self.std_dev),
            ("min", self.min),
            ("max", self.max),
        ] {
            value
                .map(|value| scale(value, factor))
                .insert(variant, measure, metric, 1.0);
        }
        for Percentile { percentile, value } in &self.percentiles {
            let Some(metric) = percentile_name(&percentile.to_string()) else {
                variant.skip_metric();
                continue;
            };
            value
                .map(|value| scale(value, factor))
                .insert(variant, measure, &metric, 1.0);
        }
    }
}

/// HTTP `RetCodes`: socket errors as `failed-requests`, 0 when there are none, each status code
/// as `responses-<code>`, built in for a code IANA assigns, else named at run time, and each
/// class's sum for the classes that hold a code.
fn insert_status_codes(variant: &mut Variant<'_>, ret_codes: &BTreeMap<String, Finite>) {
    let mut failed = Finite::from(Some(0.0));
    let mut classes: [Option<f64>; 5] = [None; 5];
    for (code, count) in ret_codes {
        if code == SOCKET_ERROR {
            failed = *count;
            continue;
        }
        let Ok(code) = code.parse::<u16>() else {
            variant.skip_measure();
            continue;
        };
        let Some(measure) = Responses::name_id(code)
            .or_else(|| variant.named(&Responses::slug(code), Responses::UNITS_STR))
        else {
            continue;
        };
        count.insert(variant, &measure, VALUE, 1.0);
        if let Some(count) = count.get()
            && let Some(class) = class(code).and_then(|class| classes.get_mut(class))
        {
            *class.get_or_insert(0.0) += count;
        }
    }
    failed.insert(variant, &FailedRequests::name_id(), VALUE, 1.0);
    for (measure, count) in [
        Responses1xx::name_id(),
        Responses2xx::name_id(),
        Responses3xx::name_id(),
        Responses4xx::name_id(),
        Responses5xx::name_id(),
    ]
    .into_iter()
    .zip(classes)
    {
        if let Some(count) = count {
            variant.insert(measure, VALUE, count);
        }
    }
}

/// The index of a status code's class among 1xx to 5xx, if it has one.
fn class(code: u16) -> Option<usize> {
    match code {
        100..=199 => Some(0),
        200..=299 => Some(1),
        300..=399 => Some(2),
        400..=499 => Some(3),
        500..=599 => Some(4),
        _ => None,
    }
}

/// A TCP, UDP, or gRPC run's failures: every call whose code is not `OK` or `SERVING`. Its codes
/// are error texts, gRPC health states, and `ERROR`, never a gRPC status, even for a ping
/// (`fgrpc/grpcrunner.go:96-128`), so they name no status code.
fn insert_failures(variant: &mut Variant<'_>, ret_codes: &BTreeMap<String, Finite>) {
    let failed = ret_codes
        .iter()
        .filter(|(code, _)| !matches!(code.as_str(), "OK" | "SERVING"))
        .map(|(_, count)| count.get())
        .sum::<Option<f64>>();
    Finite::from(failed).insert(variant, &FailedRequests::name_id(), VALUE, 1.0);
}

#[cfg(test)]
mod test_http_fortio {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{Latency, Throughput},
                http::{
                    ConnectingTime, ConnectionsOpened, FailedRequests, Requests, Responses1xx,
                    Responses2xx, Responses3xx, Responses4xx, Responses5xx, SizePerRequest,
                },
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_units, assert_value, assert_variant,
            convert_edited_v1, convert_v1, value, variants,
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const LEAF: Adapter = Adapter::HttpFortio;
    const MIXED: &str = "http://localhost:8080/mixed";
    const MIXED_RUN: &str = r#"{"@compression":false,"@duration":"5s","@h2":false,"@http1.0":false,"@jitter":false,"@keepalive":true,"@nocatchup":false,"@qps":"100","@runType":"http","@stdclient":false,"@threads":10,"@timeout":1000000000,"@uniform":false}"#;
    const MIXED_ERRORS: &str = r#"{"@compression":false,"@duration":"5s","@h2":false,"@http1.0":false,"@jitter":false,"@keepalive":true,"@nocatchup":false,"@qps":"100","@responses":"error","@runType":"http","@stdclient":false,"@threads":10,"@timeout":1000000000,"@uniform":false}"#;
    const MIXED_RECEIVED: &str = r#"{"@compression":false,"@direction":"received","@duration":"5s","@h2":false,"@http1.0":false,"@jitter":false,"@keepalive":true,"@nocatchup":false,"@qps":"100","@runType":"http","@stdclient":false,"@threads":10,"@timeout":1000000000,"@uniform":false}"#;
    const TCP_RUN: &str = r#"{"@duration":"3s","@jitter":false,"@nocatchup":false,"@qps":"200","@runType":"tcp","@threads":10,"@uniform":false}"#;
    const UDP_RUN: &str = r#"{"@duration":"3s","@jitter":false,"@nocatchup":false,"@qps":"200","@runType":"udp","@threads":10,"@uniform":false}"#;
    const PING_RUN: &str = r#"{"@duration":"3s","@jitter":false,"@nocatchup":false,"@qps":"200","@runType":"grpc_ping","@threads":10,"@uniform":false}"#;
    const HEALTH_RUN: &str = r#"{"@duration":"3s","@jitter":false,"@nocatchup":false,"@qps":"200","@runType":"grpc_health","@service":"","@threads":10,"@uniform":false}"#;
    const UNKNOWN_RUN: &str = r#"{"@duration":"3s","@jitter":false,"@nocatchup":false,"@qps":"200","@runType":"grpc_health","@service":"nosuch","@threads":10,"@uniform":false}"#;
    const UNKNOWN_ERRORS: &str = r#"{"@duration":"3s","@jitter":false,"@nocatchup":false,"@qps":"200","@responses":"error","@runType":"grpc_health","@service":"nosuch","@threads":10,"@uniform":false}"#;

    fn read(suffix: &str) -> AdapterResults {
        convert_v1(LEAF, &format!("http/fortio/{suffix}"))
    }

    fn edited<F: FnOnce(&mut serde_json::Value)>(suffix: &str, edit: F) -> AdapterResults {
        convert_edited_v1(LEAF, &format!("http/fortio/{suffix}"), edit)
    }

    /// A histogram's mean, standard deviation, min, max, and Fortio's default percentiles.
    fn histogram(measure: &'static str, values: [f64; 9]) -> Expected {
        (
            measure,
            [
                "mean", "std_dev", "min", "max", "median", "p75", "p90", "p99", "p99_9",
            ]
            .into_iter()
            .zip(values)
            .collect(),
        )
    }

    fn count(measure: &'static str, value: f64) -> Expected {
        (measure, vec![("value", value)])
    }

    /// Kills a time left in seconds, `Avg` and `StdDev` swapped, a percentile misnamed, `Sum`,
    /// `ActualDuration`, or `HeaderSizes` written, the error histogram's statistics on the run's
    /// variant or the run's on its `@responses` variant, the sizes without their direction,
    /// `-1` written as a status code or summed into a class, a class missing or summed into
    /// another, `ConnectionStats` or `SocketCount` dropped, and a unit missing.
    #[test]
    fn adapter_http_fortio_measures() {
        let results = read("mixed_timeout.json");
        assert_eq!(results.resolved_adapter, Some(LEAF));
        assert!(results.nameless.is_none());
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(variants(&results), 3);
        assert_variant(
            &results,
            MIXED,
            MIXED_RUN,
            &[
                histogram(
                    Latency::SLUG_STR,
                    [
                        57_043_053.404_545_456,
                        231_633_395.490_785_1,
                        12_609.0,
                        1_001_473_262.0,
                        541_309.804_400_978,
                        806_867.286_063_569_7,
                        966_201.775_061_124_6,
                        1_001_213_967.887_999_9,
                        1_001_447_332.588_8,
                    ],
                ),
                (Throughput::SLUG_STR, vec![("mean", 76.141_463_277_923_08)]),
                count(Requests::SLUG_STR, 440.0),
                count(FailedRequests::SLUG_STR, 16.0),
                count("responses-200", 347.0),
                count("responses-404", 33.0),
                count("responses-500", 44.0),
                count(Responses2xx::SLUG_STR, 347.0),
                count(Responses4xx::SLUG_STR, 33.0),
                count(Responses5xx::SLUG_STR, 44.0),
                histogram(
                    ConnectingTime::SLUG_STR,
                    [
                        122_325.0,
                        68_370.900_528_802_47,
                        23_886.0,
                        629_766.0,
                        324_548.255_639_097_75,
                        477_157.127_819_548_9,
                        568_722.451_127_819_6,
                        623_661.645_112_782,
                        629_155.564_511_278_3,
                    ],
                ),
                count(ConnectionsOpened::SLUG_STR, 134.0),
            ],
        );
        assert_variant(
            &results,
            MIXED,
            MIXED_ERRORS,
            &[histogram(
                Latency::SLUG_STR,
                [
                    161_622_610.064_516_14,
                    367_990_757.758_236_3,
                    49_746.0,
                    1_001_239_115.0,
                    611_259.727_272_727_2,
                    898_187.071_428_571_4,
                    1_000_470_863.7,
                    1_001_162_289.869_999_9,
                    1_001_231_432.487,
                ],
            )],
        );
        assert_variant(
            &results,
            MIXED,
            MIXED_RECEIVED,
            &[(
                SizePerRequest::SLUG_STR,
                vec![
                    ("mean", 109.397_727_272_727_27),
                    ("std_dev", 24.246_995_958_603_375),
                    ("min", 0.0),
                    ("max", 119.0),
                ],
            )],
        );
        assert_units(&results, &[]);
    }

    /// Kills an unregistered code dropped or left out of its class, 418 left out of 4xx though
    /// Fortio counts it as OK, two codes of one class not summed, a 3xx code left out of 3xx or
    /// summed into another class, and no socket error not written as 0.
    #[test]
    fn adapter_http_fortio_status_codes() {
        let results = read("status_codes.json");
        assert_eq!(results.skipped, Skipped::default());
        let run = r#"{"@compression":false,"@duration":"3s","@h2":false,"@http1.0":false,"@jitter":false,"@keepalive":true,"@nocatchup":false,"@qps":"200","@runType":"http","@stdclient":false,"@threads":10,"@timeout":3000000000,"@uniform":false}"#;
        let name = "http://localhost:8078/echo?status=599:10,418:10,201:20,200:60";
        for (measure, expected) in [
            (FailedRequests::SLUG_STR, 0.0),
            ("responses-200", 344.0),
            ("responses-201", 131.0),
            ("responses-418", 62.0),
            ("responses-599", 63.0),
            (Responses2xx::SLUG_STR, 475.0),
            (Responses4xx::SLUG_STR, 62.0),
            (Responses5xx::SLUG_STR, 63.0),
            (Requests::SLUG_STR, 600.0),
        ] {
            assert_value(&results, (name, run), (measure, "value"), expected);
        }
        assert_eq!(
            value(&results, name, run, Responses1xx::SLUG_STR, "value"),
            None
        );
        assert_units(&results, &[("responses-599", "responses")]);
        assert_value(
            &results,
            (name, &run.replacen('{', r#"{"@responses":"error","#, 1)),
            (Latency::SLUG_STR, "max"),
            1_969_116.0,
        );
        let redirects = read("codes3xx.json");
        let run = run.replace(r#""3s""#, r#""2s""#);
        let name = "http://localhost:18078/echo?status=302:30,304:20,200:50";
        for (measure, expected) in [
            (Responses2xx::SLUG_STR, 216.0),
            (Responses3xx::SLUG_STR, 184.0),
        ] {
            assert_value(&redirects, (name, &run), (measure, "value"), expected);
        }
    }

    /// Kills Fortio's own default label, which holds the host's name, kept as the name, a user's
    /// label replaced by the target, the scheme not stripped, or stripped in any case, before the
    /// label is compared, a host with a `.` read as a default label, and a non-HTTP run named by
    /// anything but its destination.
    #[test]
    fn adapter_http_fortio_names() {
        for (suffix, name) in [
            ("mixed_timeout.json", MIXED),
            ("mixed_exactly_labels.json", "checkout api"),
            ("h2.json", "https://localhost:8443/fast"),
            ("tcp.json", "tcp://localhost:8077"),
            ("udp.json", "udp://localhost:8076"),
            ("grpc_ping.json", "localhost:8079"),
        ] {
            let results = read(suffix);
            assert_eq!(
                results
                    .inner
                    .keys()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                vec![name.to_owned()],
                "{suffix}"
            );
        }
        for (labels, name) in [
            ("localhost:8080/mixed , ci-runner-7", MIXED),
            ("localhost:8080/mixed , ", MIXED),
            (
                "localhost:8080/mixed , runner.example",
                "localhost:8080/mixed , runner.example",
            ),
            (
                "http://localhost:8080/mixed , localhost",
                "http://localhost:8080/mixed , localhost",
            ),
        ] {
            let results = edited("mixed_timeout.json", |json| {
                json["Labels"] = labels.into();
            });
            assert!(
                value(&results, name, MIXED_RUN, Requests::SLUG_STR, "value").is_some(),
                "{labels}"
            );
        }
        let results = edited("mixed_timeout.json", |json| {
            json["URL"] = "HTTP://localhost:8080/mixed".into();
            json["Labels"] = "HTTP://localhost:8080/mixed , localhost".into();
        });
        assert!(
            value(
                &results,
                "HTTP://localhost:8080/mixed",
                MIXED_RUN,
                Requests::SLUG_STR,
                "value"
            )
            .is_some()
        );
    }

    /// Kills `@keepalive` not the inverse of `DisableKeepAlive`, `@exactly` and `@duration`
    /// both written or the wrong one, a setting read from another field, and an HTTP setting
    /// written for a run that has none.
    #[test]
    fn adapter_http_fortio_parameters() {
        for (suffix, name, parameters) in [
            (
                "fast_defaults.json",
                "http://localhost:8080/fast",
                r#"{"@compression":false,"@duration":"5s","@h2":false,"@http1.0":false,"@jitter":false,"@keepalive":true,"@nocatchup":false,"@qps":"8","@runType":"http","@stdclient":false,"@threads":4,"@timeout":3000000000,"@uniform":false}"#,
            ),
            (
                "fast_max_qps.json",
                "http://localhost:8080/fast",
                r#"{"@compression":false,"@duration":"3s","@h2":false,"@http1.0":false,"@jitter":false,"@keepalive":true,"@nocatchup":false,"@qps":"max","@runType":"http","@stdclient":false,"@threads":10,"@timeout":3000000000,"@uniform":false}"#,
            ),
            (
                "mixed_exactly_labels.json",
                "checkout api",
                r#"{"@compression":false,"@exactly":500,"@h2":false,"@http1.0":false,"@jitter":false,"@keepalive":true,"@nocatchup":false,"@qps":"100","@runType":"http","@stdclient":false,"@threads":10,"@timeout":3000000000,"@uniform":false}"#,
            ),
            (
                "h2.json",
                "https://localhost:8443/fast",
                r#"{"@compression":false,"@duration":"3s","@h2":true,"@http1.0":false,"@jitter":false,"@keepalive":true,"@nocatchup":false,"@qps":"200","@runType":"http","@stdclient":true,"@threads":10,"@timeout":3000000000,"@uniform":false}"#,
            ),
            (
                "uniform_jitter.json",
                "http://localhost:8080/fast",
                r#"{"@compression":false,"@duration":"3s","@h2":false,"@http1.0":false,"@jitter":true,"@keepalive":true,"@nocatchup":false,"@qps":"200","@runType":"http","@stdclient":false,"@threads":10,"@timeout":3000000000,"@uniform":true}"#,
            ),
            ("tcp.json", "tcp://localhost:8077", TCP_RUN),
            ("grpc_ping.json", "localhost:8079", PING_RUN),
        ] {
            let results = read(suffix);
            assert!(
                value(&results, name, parameters, Requests::SLUG_STR, "value").is_some(),
                "{suffix}"
            );
        }
        for (key, flag, parameter, expected) in [
            ("DisableKeepAlive", true, "@keepalive", false),
            ("HTTP10", true, "@http1.0", true),
            ("Compression", true, "@compression", true),
            ("NoCatchUp", true, "@nocatchup", true),
        ] {
            let results = edited("fast_defaults.json", |json| {
                json[key] = flag.into();
            });
            let parameters = results
                .inner
                .values()
                .next()
                .unwrap()
                .keys()
                .next()
                .unwrap();
            let parameters = serde_json::to_value(parameters).unwrap();
            assert_eq!(parameters[parameter], expected, "{key}");
        }
    }

    /// Kills a secret, the payload, the gRPC metadata, a certificate, `Resolve`, `ID`, or the
    /// labels read as a parameter: a change to any leaves the results as they were.
    #[test]
    fn adapter_http_fortio_never_reads_secrets() {
        let results = read("post_secret.json");
        for (key, junk) in [
            ("UserCredentials", serde_json::json!("bob:other")),
            ("Payload", serde_json::json!("eQ==")),
            ("ContentType", serde_json::json!("text/plain")),
            ("MethodOverride", serde_json::json!("PUT")),
            ("CACert", serde_json::json!("/ca.pem")),
            ("Cert", serde_json::json!("/cert.pem")),
            ("Key", serde_json::json!("/key.pem")),
            ("MTLS", serde_json::json!(true)),
            ("Insecure", serde_json::json!(true)),
            ("Resolve", serde_json::json!("10.0.0.1")),
            ("ID", serde_json::json!("other")),
            ("UnixDomainSocket", serde_json::json!("/sock")),
        ] {
            let edited = edited("post_secret.json", |json| {
                json[key] = junk.clone();
            });
            assert_eq!(edited, results, "{key}");
        }
        let grpc = read("grpc_ping.json");
        let edited = edited("grpc_ping.json", |json| {
            json["Metadata"] = serde_json::json!({ "authorization": ["Bearer OTHER"] });
        });
        assert_eq!(edited, grpc);
        let labeled = read("mixed_exactly_labels.json");
        for variants in labeled.inner.values() {
            for parameters in variants.keys() {
                let parameters = serde_json::to_value(parameters).unwrap();
                for key in ["@labels", "labels", "Labels", "@id"] {
                    assert!(parameters.get(key).is_none(), "{parameters}");
                }
            }
        }
    }

    /// Kills a TCP or UDP error text written as a status measure, `OK` counted as a failure, no
    /// failure not written as 0, `SocketCount` dropped, the error histogram dropped, and a TCP or
    /// UDP run's `ReqTimeout` written as `@timeout`, though Fortio never records it.
    #[test]
    fn adapter_http_fortio_socket_runs() {
        let timeout = read("tcp_timeout.json");
        assert_eq!(variants(&timeout), 1);
        for parameters in timeout.inner.values().flat_map(|variants| variants.keys()) {
            let parameters = serde_json::to_value(parameters).unwrap();
            assert_eq!(parameters["@runType"], "tcp");
            assert!(parameters.get("@timeout").is_none(), "{parameters}");
        }
        let refused = read("tcp_refused.json");
        assert_eq!(refused.skipped, Skipped::default());
        assert_units(&refused, &[]);
        let name = "tcp://localhost:9";
        assert_variant(
            &refused,
            name,
            TCP_RUN,
            &[
                histogram(
                    Latency::SLUG_STR,
                    [
                        143_588.37,
                        181_872.673_644_667_6,
                        15_524.0,
                        1_951_280.0,
                        512_750.898_648_648_7,
                        762_195.831_081_081_1,
                        911_862.790_540_540_6,
                        1_135_897.142_857_146,
                        1_869_741.714_285_719_2,
                    ],
                ),
                (Throughput::SLUG_STR, vec![("mean", 199.981_514_042_144_48)]),
                count(Requests::SLUG_STR, 600.0),
                count(FailedRequests::SLUG_STR, 600.0),
                count(ConnectionsOpened::SLUG_STR, 610.0),
            ],
        );
        let errors = TCP_RUN.replacen('{', r#"{"@responses":"error","#, 1);
        assert_value(
            &refused,
            (name, &errors),
            (Latency::SLUG_STR, "mean"),
            143_588.37,
        );
        let udp = read("udp.json");
        assert_eq!(variants(&udp), 1);
        for (measure, expected) in [
            (FailedRequests::SLUG_STR, 0.0),
            (ConnectionsOpened::SLUG_STR, 10.0),
            (Requests::SLUG_STR, 600.0),
        ] {
            assert_value(
                &udp,
                ("udp://localhost:8076", UDP_RUN),
                (measure, "value"),
                expected,
            );
        }
        let short_read = read("udp_refused.json");
        assert_value(
            &short_read,
            ("udp://localhost:9", UDP_RUN),
            (FailedRequests::SLUG_STR, "value"),
            600.0,
        );
    }

    /// Kills `SERVING` counted as a failure, `ERROR` or a health state not counted, a ping's
    /// `SERVING` or a health state written as a status measure, and an empty error histogram
    /// written or counted.
    #[test]
    fn adapter_http_fortio_grpc_runs() {
        let ping = read("grpc_ping.json");
        assert_eq!(ping.skipped, Skipped::default());
        assert_variant(
            &ping,
            "localhost:8079",
            PING_RUN,
            &[
                histogram(
                    Latency::SLUG_STR,
                    [
                        287_361.868_333_333_3,
                        116_860.881_197_463_47,
                        91_692.0,
                        1_160_778.0,
                        547_370.006_711_409_5,
                        775_971.013_422_818_7,
                        913_131.617_449_664_5,
                        995_427.979_865_772,
                        1_128_622.400_000_001_8,
                    ],
                ),
                (Throughput::SLUG_STR, vec![("mean", 199.956_205_325_301_4)]),
                count(Requests::SLUG_STR, 600.0),
                count(FailedRequests::SLUG_STR, 0.0),
            ],
        );
        assert_eq!(variants(&ping), 1);
        let health = read("grpc_health.json");
        assert_value(
            &health,
            ("localhost:8079", HEALTH_RUN),
            (FailedRequests::SLUG_STR, "value"),
            0.0,
        );
        let unknown = read("grpc_health_unknown.json");
        assert_units(&unknown, &[]);
        assert_value(
            &unknown,
            ("localhost:8079", UNKNOWN_RUN),
            (FailedRequests::SLUG_STR, "value"),
            600.0,
        );
        assert_value(
            &unknown,
            ("localhost:8079", UNKNOWN_ERRORS),
            (Latency::SLUG_STR, "max"),
            3_338_930.0,
        );
        let not_serving = edited("grpc_health.json", |json| {
            json["RetCodes"] = serde_json::json!({ "SERVING": 590, "NOT_SERVING": 7, "ERROR": 3 });
        });
        assert_value(
            &not_serving,
            ("localhost:8079", HEALTH_RUN),
            (FailedRequests::SLUG_STR, "value"),
            10.0,
        );
    }

    /// Kills one `@runType` for 2 run types, a ping and a health check's above all, a ping with a
    /// delay or a payload not read as a ping, `@service` dropped or written for a run that checks
    /// no service, and the service read up to its first `'`, with the payload's length that
    /// follows it, cut at a ` PayloadLength=` in its name when the run sends no payload, or cut
    /// at the first ` PayloadLength=` in place of the last when it does.
    #[test]
    fn adapter_http_fortio_run_types() {
        for (suffix, run_type, service) in [
            ("mixed_timeout.json", "http", None),
            ("tcp_refused.json", "tcp", None),
            ("udp.json", "udp", None),
            ("grpc_ping.json", "grpc_ping", None),
            ("ping_delay.json", "grpc_ping", None),
            ("grpc_health.json", "grpc_health", Some("")),
            ("grpc_health_unknown.json", "grpc_health", Some("nosuch")),
            ("grpc_health_payload.json", "grpc_health", Some("")),
            ("grpc_health_quote.json", "grpc_health", Some("it's")),
            (
                "grpc_health_payload_name.json",
                "grpc_health",
                Some("a' PayloadLength=5"),
            ),
            (
                "name_payload.json",
                "grpc_health",
                Some("a' PayloadLength=5"),
            ),
        ] {
            let results = read(suffix);
            assert_eq!(results.skipped, Skipped::default(), "{suffix}");
            assert!(variants(&results) > 0, "{suffix}");
            for parameters in results.inner.values().flat_map(|variants| variants.keys()) {
                let parameters = serde_json::to_value(parameters).unwrap();
                assert_eq!(parameters["@runType"], run_type, "{suffix}");
                assert_eq!(
                    parameters
                        .get("@service")
                        .and_then(serde_json::Value::as_str),
                    service,
                    "{suffix}"
                );
            }
        }
    }

    /// Kills an empty histogram's zeros written as measured: the run's latency, its connection
    /// times, and its sizes, with its counts still written.
    #[test]
    fn adapter_http_fortio_empty_histograms() {
        let empty = serde_json::json!({
            "Count": 0, "Min": 0, "Max": 0, "Sum": 0, "Avg": 0, "StdDev": 0, "Data": null
        });
        let results = edited("fast.json", |json| {
            for key in ["DurationHistogram", "ConnectionStats", "Sizes"] {
                json[key] = empty.clone();
            }
        });
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(variants(&results), 1);
        let run = r#"{"@compression":false,"@duration":"3s","@h2":false,"@http1.0":false,"@jitter":false,"@keepalive":true,"@nocatchup":false,"@qps":"500","@runType":"http","@stdclient":false,"@threads":10,"@timeout":3000000000,"@uniform":false}"#;
        let name = "http://localhost:8080/fast";
        for measure in [Latency::SLUG_STR, ConnectingTime::SLUG_STR] {
            assert_eq!(
                value(&results, name, run, measure, "max"),
                None,
                "{measure}"
            );
        }
        assert_value(&results, (name, run), (Requests::SLUG_STR, "value"), 0.0);
    }

    /// Kills a leaf that claims several documents, a run that aborted before it started, a
    /// custom gRPC method's run, which counts each failure twice, or a run type it does not know.
    #[test]
    fn adapter_http_fortio_refuses() {
        let settings = Settings::new(None, BmfVersion::V1);
        let concat = std::fs::read_to_string("./tool_output/http/fortio/concat.json")
            .expect("Failed to read a fixture");
        assert_eq!(LEAF.convert(&concat, settings), None);
        for (suffix, key, junk) in [
            ("fast.json", "error", serde_json::json!(true)),
            (
                "grpc_ping.json",
                "RunType",
                serde_json::json!("Custom GRPC Method fgrpc.PingServer/Ping and Payload {}"),
            ),
            ("tcp.json", "RunType", serde_json::json!("SCTP")),
            ("fast.json", "RunType", serde_json::json!("TCP")),
        ] {
            let input = std::fs::read_to_string(format!("./tool_output/http/fortio/{suffix}"))
                .expect("Failed to read a fixture");
            assert!(LEAF.convert(&input, settings).is_some(), "{suffix}");
            let mut json = serde_json::from_str::<serde_json::Value>(&input).unwrap();
            json[key] = junk;
            assert_eq!(
                LEAF.convert(&json.to_string(), settings),
                None,
                "{suffix} {key}"
            );
        }
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_http_fortio_ignores_average() {
        assert_ignores_average(LEAF, "http/fortio/mixed_timeout.json");
    }
}
