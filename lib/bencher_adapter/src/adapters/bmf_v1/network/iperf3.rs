use std::{collections::BTreeMap, fmt};

use bencher_json::{
    BenchmarkName, MeasureNameId,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            systems::{
                BitsThroughput, CongestionWindow, CpuUtilization, LostPackets, OutOfOrderPackets,
                PacketJitter, PacketLoss, Retransmits, RoundTripTime, SendWindow,
            },
        },
        report::Adapter,
    },
};
use serde::{
    Deserialize, Deserializer,
    de::{Error as _, MapAccess, Visitor},
};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::leaf::{LeafResults, parameter_set},
    results::adapter_results::AdapterResults,
};

const TCP: &str = "TCP";
const NANOS_PER_MICRO: f64 = 1e3;
const NANOS_PER_MILLI: f64 = 1e6;

/// iperf3's `-J` report of a client run, its intervals unread.
///
/// A run with an `error` is refused even when its `end` holds sums: iperf3 sets `error` only
/// when the test did not finish, and the side that never reported is written as zeros. A
/// server's report is refused too: its sender rate is 0, and its local CPU is the server's.
pub struct AdapterNetworkIperf3;

impl Adaptable for AdapterNetworkIperf3 {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let report = serde_json::from_str::<Report>(input).ok()?;
        if !report.start.version.starts_with("iperf 3.") || report.error.is_some() {
            return None;
        }
        let host = report.start.connecting_to.as_ref()?.host.as_str();
        let name = report.title.as_deref().unwrap_or(host);
        let test = report.start.test_start.as_ref()?;
        let end = &report.end;
        let forward = if test.reverse == 0 {
            Direction::ClientToServer
        } else {
            Direction::ServerToClient
        };
        let flows = end.flows(forward);
        if flows.is_empty() {
            return None;
        }
        let mut results = LeafResults::default();
        let Ok(benchmark) = name.parse::<BenchmarkName>() else {
            results.skip_benchmark();
            return results.finish_with_skips(Adapter::NetworkIperf3);
        };
        // TCP's unlimited rate is 0 where iperf3 from 3.7 to 3.10 wrote none, so a series stays
        // whole across 3.11.
        let fallback = report
            .start
            .target_bitrate
            .or_else(|| (test.protocol == TCP && end.flags_senders()).then_some(0));
        let settings = test.parameters(fallback, &flows);
        let mut write = |extra: &[(&str, &str)], metrics: Metrics| {
            if metrics.is_empty() {
                return;
            }
            let mut parameters = settings.clone();
            for (key, value) in extra {
                parameters.insert((*key).to_owned(), (*value).into());
            }
            let Some(parameters) = parameter_set(parameters) else {
                results.skip_variant();
                return;
            };
            let mut variant = results.variant(benchmark.clone(), parameters);
            for (measure, metric, value) in metrics {
                variant.insert(measure, metric, value);
            }
        };
        for flow in &flows {
            let direction = ("@direction", flow.direction.as_str());
            let mut sender = vec![(BitsThroughput::name_id(), "mean", flow.sent.bits_per_second)];
            if let Some(retransmits) = flow.sent.retransmits {
                sender.push((Retransmits::name_id(), "value", retransmits));
            }
            // SCTP's RTT is the kernel's `spinfo_srtt`, in milliseconds (`iperf_sctp.c:760`).
            if flow.direction == Direction::ClientToServer && test.protocol == TCP {
                end.insert_tcp_state(&mut sender, forward);
            }
            write(&[direction, ("@side", "sender")], sender);
            write(
                &[direction, ("@side", "receiver")],
                end.receiver(flow, forward),
            );
        }
        if let Some(cpu) = &end.cpu_utilization_percent {
            for (host, [user, system, total]) in [
                (None, [cpu.host_user, cpu.host_system, cpu.host_total]),
                (
                    Some("remote"),
                    [cpu.remote_user, cpu.remote_system, cpu.remote_total],
                ),
            ] {
                for (mode, value) in [("user", user), ("system", system), ("total", total)] {
                    let mut extra = vec![("@cpu", mode)];
                    extra.extend(host.map(|host| ("@host", host)));
                    write(&extra, vec![(CpuUtilization::name_id(), "value", value)]);
                }
            }
        }
        results.finish_with_skips(Adapter::NetworkIperf3)
    }
}

type Metrics = Vec<(MeasureNameId, &'static str, f64)>;

/// A report, whose `error` iperf3 writes when the test did not finish.
#[derive(Deserialize)]
struct Report {
    start: Begin,
    #[serde(deserialize_with = "End::pair_bidir_sums")]
    end: End,
    title: Option<String>,
    error: Option<String>,
}

/// The run's start, with `connecting_to` only in a client's report, and from iperf3 3.7 to 3.10
/// a TCP client's `target_bitrate` only when `-b` set one (`iperf_api.c:904-905` at 3.10).
#[derive(Deserialize)]
struct Begin {
    version: String,
    connecting_to: Option<ConnectingTo>,
    target_bitrate: Option<u64>,
    test_start: Option<TestStart>,
}

#[derive(Deserialize)]
struct ConnectingTo {
    host: String,
}

/// The run's resolved settings, with `target_bitrate` from iperf3 3.11.
#[derive(Deserialize)]
struct TestStart {
    protocol: String,
    num_streams: u64,
    blksize: u64,
    omit: u64,
    duration: u64,
    bytes: u64,
    blocks: u64,
    reverse: u8,
    target_bitrate: Option<u64>,
}

impl TestStart {
    /// The settings every variant carries, with the one end condition iperf3 allows, `fallback`
    /// where this has no rate (before iperf3 3.11), and `@bidir` when the run had a flow each way.
    fn parameters(
        &self,
        fallback: Option<u64>,
        flows: &[Flow<'_>],
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut parameters = serde_json::Map::new();
        parameters.insert("@protocol".to_owned(), self.protocol.clone().into());
        parameters.insert("@parallel".to_owned(), self.num_streams.into());
        let (end, value) = if self.bytes > 0 {
            ("@bytes", self.bytes)
        } else if self.blocks > 0 {
            ("@blocks", self.blocks)
        } else {
            ("@time", self.duration)
        };
        parameters.insert(end.to_owned(), value.into());
        if let Some(bitrate) = self.target_bitrate.or(fallback) {
            parameters.insert("@bitrate".to_owned(), bitrate.into());
        }
        parameters.insert("@length".to_owned(), self.blksize.into());
        if self.omit > 0 {
            parameters.insert("@omit".to_owned(), self.omit.into());
        }
        if flows.len() > 1 {
            parameters.insert("@bidir".to_owned(), true.into());
        }
        parameters
    }
}

/// The run's end, with UDP's `sum` alone before iperf3 3.11.
#[derive(Deserialize)]
struct End {
    #[serde(default)]
    streams: Vec<Stream>,
    sum_sent: Option<Sum>,
    sum_received: Option<Sum>,
    sum: Option<Sum>,
    sum_sent_bidir_reverse: Option<Sum>,
    sum_received_bidir_reverse: Option<Sum>,
    sum_bidir_reverse: Option<Sum>,
    cpu_utilization_percent: Option<Cpu>,
}

impl End {
    /// The end, with each sum iperf3 before 3.11 writes twice under one key for a `--bidir` run
    /// (`iperf_api.c:3662-3714` at 3.10) paired by its `sender` value: the client's flow keeps
    /// the key, and the server's takes the `_bidir_reverse` key that iperf3 3.11 gives it.
    fn pair_bidir_sums<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut end = serde_json::Map::new();
        for (key, copies) in deserializer.deserialize_map(Copies)? {
            match <[_; 1]>::try_from(copies) {
                Ok([only]) => {
                    end.insert(key, only);
                },
                Err(copies) => {
                    let (client, server): (Vec<_>, Vec<_>) = copies.into_iter().partition(|copy| {
                        copy.get("sender") == Some(&serde_json::Value::Bool(true))
                    });
                    let (Ok([client]), Ok([server])) =
                        (<[_; 1]>::try_from(client), <[_; 1]>::try_from(server))
                    else {
                        return Err(D::Error::custom(format!(
                            "`{key}` is repeated without one copy for each flow"
                        )));
                    };
                    end.insert(format!("{key}_bidir_reverse"), server);
                    end.insert(key, client);
                },
            }
        }
        Self::deserialize(serde_json::Value::Object(end)).map_err(D::Error::custom)
    }

    /// The run's flows, named by `-R` and the `*_bidir_reverse` keys, since a UDP `sum_sent`
    /// always says `sender: true` (`iperf_api.c:4689`).
    fn flows(&self, forward: Direction) -> Vec<Flow<'_>> {
        [
            (forward, &self.sum_sent, &self.sum_received, &self.sum),
            (
                Direction::ServerToClient,
                &self.sum_sent_bidir_reverse,
                &self.sum_received_bidir_reverse,
                &self.sum_bidir_reverse,
            ),
        ]
        .into_iter()
        .filter_map(|(direction, sent, received, sum)| {
            Some(Flow {
                direction,
                sent: sent.as_ref().or(sum.as_ref())?,
                received: received.as_ref(),
                losses: [received, sum]
                    .into_iter()
                    .flatten()
                    .find(|losses| losses.jitter_ms.is_some()),
            })
        })
        .collect()
    }

    /// Whether each TCP stream names its sending side, a flag iperf3 3.7 added with the `-b` rate
    /// (`iperf_api.c:746` and `:3135` at 3.7), so a run without it has no known rate.
    fn flags_senders(&self) -> bool {
        self.streams
            .iter()
            .filter_map(|stream| stream.tcp.as_ref())
            .all(|stream| stream.sender.is_some())
    }

    /// The RTT (in microseconds, `tcp_info.c:195`) and windows of the TCP streams the client
    /// sent on, past any stream iperf3 never sampled, which it writes as zeros
    /// (`iperf_api.c:3934-3939`).
    fn insert_tcp_state(&self, metrics: &mut Metrics, forward: Direction) {
        let streams: Vec<&TcpStream> = self
            .streams
            .iter()
            .filter_map(|stream| stream.tcp.as_ref())
            .filter(|stream| stream.sender.unwrap_or(forward.client_sends()))
            .collect();
        let sampled: Vec<[f64; 3]> = streams
            .iter()
            .filter_map(|stream| Some([stream.min_rtt?, stream.max_rtt?, stream.mean_rtt?]))
            .filter(|[_, max, _]| *max > 0.0)
            .collect();
        if let Some(min) = sampled.iter().map(|[min, _, _]| *min).reduce(f64::min)
            && let Some(max) = sampled.iter().map(|[_, max, _]| *max).reduce(f64::max)
        {
            let measure = RoundTripTime::name_id();
            metrics.push((measure.clone(), "min", min * NANOS_PER_MICRO));
            metrics.push((measure.clone(), "max", max * NANOS_PER_MICRO));
            if let ([[_, _, mean]], [_]) = (sampled.as_slice(), streams.as_slice()) {
                metrics.push((measure, "mean", mean * NANOS_PER_MICRO));
            }
        }
        for (measure, window) in [
            (
                CongestionWindow::name_id(),
                streams
                    .iter()
                    .filter_map(|stream| stream.max_snd_cwnd)
                    .reduce(f64::max),
            ),
            (
                SendWindow::name_id(),
                streams
                    .iter()
                    .filter_map(|stream| stream.max_snd_wnd)
                    .reduce(f64::max),
            ),
        ] {
            if let Some(window) = window
                && window > 0.0
            {
                metrics.push((measure, "max", window));
            }
        }
    }

    /// The receiver's rate and losses, with out-of-order datagrams only where the client
    /// received, since the server's count never reaches it (`iperf_udp.c:194`), summed as
    /// iperf3 sums lost datagrams (`iperf_api.c:4422`).
    fn receiver(&self, flow: &Flow<'_>, forward: Direction) -> Metrics {
        let mut metrics = Vec::new();
        if let Some(received) = flow.received {
            metrics.push((BitsThroughput::name_id(), "mean", received.bits_per_second));
        }
        if flow.direction == Direction::ServerToClient {
            let out_of_order: Vec<f64> = self
                .streams
                .iter()
                .filter_map(|stream| stream.udp.as_ref())
                .filter(|stream| !stream.sender.unwrap_or(forward.client_sends()))
                .filter_map(|stream| stream.out_of_order)
                .collect();
            if !out_of_order.is_empty() {
                metrics.push((
                    OutOfOrderPackets::name_id(),
                    "value",
                    out_of_order.iter().sum(),
                ));
            }
        }
        if let Some(losses) = flow.losses {
            for (measure, value) in [
                (
                    PacketJitter::name_id(),
                    losses.jitter_ms.map(|jitter| jitter * NANOS_PER_MILLI),
                ),
                (LostPackets::name_id(), losses.lost_packets),
                (PacketLoss::name_id(), losses.lost_percent),
            ] {
                if let Some(value) = value {
                    metrics.push((measure, "value", value));
                }
            }
        }
        metrics
    }
}

/// A JSON object read with every copy of a repeated key, in order.
struct Copies;

impl<'de> Visitor<'de> for Copies {
    type Value = BTreeMap<String, Vec<serde_json::Value>>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut copies = Self::Value::new();
        while let Some((key, value)) = map.next_entry::<String, serde_json::Value>()? {
            copies.entry(key).or_default().push(value);
        }
        Ok(copies)
    }
}

/// One direction of a run, whose receiver's sum starts with iperf3 3.11.
struct Flow<'a> {
    direction: Direction,
    sent: &'a Sum,
    received: Option<&'a Sum>,
    losses: Option<&'a Sum>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    ClientToServer,
    ServerToClient,
}

impl Direction {
    fn client_sends(self) -> bool {
        self == Self::ClientToServer
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::ClientToServer => "client_to_server",
            Self::ServerToClient => "server_to_client",
        }
    }
}

/// A sum of a flow's streams, with `retransmits` only where the sender's platform counts them.
#[derive(Deserialize)]
struct Sum {
    bits_per_second: f64,
    retransmits: Option<f64>,
    jitter_ms: Option<f64>,
    lost_packets: Option<f64>,
    lost_percent: Option<f64>,
}

#[derive(Deserialize)]
struct Stream {
    #[serde(rename = "sender")]
    tcp: Option<TcpStream>,
    udp: Option<UdpStream>,
}

/// A stream's sending side, with its TCP state only where the platform reports it, and before
/// iperf3 3.7 no `sender`, when only `-R` says which side sent.
#[derive(Deserialize)]
struct TcpStream {
    sender: Option<bool>,
    min_rtt: Option<f64>,
    max_rtt: Option<f64>,
    mean_rtt: Option<f64>,
    max_snd_cwnd: Option<f64>,
    max_snd_wnd: Option<f64>,
}

#[derive(Deserialize)]
struct UdpStream {
    sender: Option<bool>,
    out_of_order: Option<f64>,
}

#[derive(Deserialize)]
struct Cpu {
    host_total: f64,
    host_user: f64,
    host_system: f64,
    remote_total: f64,
    remote_user: f64,
    remote_system: f64,
}

#[cfg(test)]
mod test_network_iperf3 {
    use std::collections::BTreeSet;

    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                systems::{
                    BitsThroughput, CongestionWindow, CpuUtilization, LostPackets,
                    OutOfOrderPackets, PacketJitter, PacketLoss, Retransmits, RoundTripTime,
                    SendWindow,
                },
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;
    use serde_json::{Map, Value, json};

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_value, assert_variant, convert_edited_v1,
            convert_v1,
        },
        results::adapter_results::AdapterResults,
    };

    const IPERF3: &str = "network/iperf3";

    fn convert(fixture: &str) -> AdapterResults {
        convert_v1(Adapter::NetworkIperf3, &format!("{IPERF3}/{fixture}"))
    }

    fn convert_edited<F: FnOnce(&mut Value)>(fixture: &str, edit: F) -> AdapterResults {
        convert_edited_v1(Adapter::NetworkIperf3, &format!("{IPERF3}/{fixture}"), edit)
    }

    fn parse(input: &str) -> Option<AdapterResults> {
        Adapter::NetworkIperf3.convert(input, Settings::new(None, BmfVersion::V1))
    }

    fn read(fixture: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/{IPERF3}/{fixture}"))
            .expect("Failed to read a fixture")
    }

    /// A run's settings, as a JSON object.
    fn settings(settings: &Value) -> Map<String, Value> {
        settings
            .as_object()
            .expect("Failed to read settings")
            .clone()
    }

    /// The parameters of a run's settings with `extra` added.
    fn with(settings: &Map<String, Value>, extra: &[(&str, &str)]) -> String {
        let mut parameters = settings.clone();
        for (key, value) in extra {
            parameters.insert((*key).to_owned(), (*value).into());
        }
        Value::Object(parameters).to_string()
    }

    fn side(settings: &Map<String, Value>, direction: &str, side: &str) -> String {
        with(settings, &[("@direction", direction), ("@side", side)])
    }

    /// Every variant of each benchmark: one per side of each flow, and one per CPU mode of each
    /// host.
    fn variant_names(results: &AdapterResults) -> BTreeSet<(String, String)> {
        results
            .inner
            .iter()
            .flat_map(|(name, variants)| {
                variants
                    .keys()
                    .map(move |parameters| (name.to_string(), parameters.canonical()))
            })
            .collect()
    }

    fn expected_variants(
        name: &str,
        settings: &Map<String, Value>,
        flows: &[(&str, &[&str])],
    ) -> BTreeSet<(String, String)> {
        let mut parameters = Vec::new();
        for (direction, sides) in flows {
            for side_name in *sides {
                parameters.push(side(settings, direction, side_name));
            }
        }
        for cpu in ["user", "system", "total"] {
            parameters.push(with(settings, &[("@cpu", cpu)]));
            parameters.push(with(settings, &[("@cpu", cpu), ("@host", "remote")]));
        }
        parameters
            .into_iter()
            .map(|parameters| {
                let parameters: bencher_json::ParameterSet =
                    parameters.parse().expect("Failed to parse parameters");
                (name.to_owned(), parameters.canonical())
            })
            .collect()
    }

    const BOTH: &[&str] = &["sender", "receiver"];
    const C2S: &str = "client_to_server";
    const S2C: &str = "server_to_client";

    fn rate(bits_per_second: f64) -> Expected {
        (BitsThroughput::SLUG_STR, vec![("mean", bits_per_second)])
    }

    fn count(measure: &'static str, value: f64) -> Expected {
        (measure, vec![("value", value)])
    }

    fn tcp_settings() -> Map<String, Value> {
        settings(&json!({
            "@protocol": "TCP",
            "@parallel": 1,
            "@time": 3,
            "@bitrate": 0,
            "@length": 128 * 1024,
        }))
    }

    /// Kills: the sender's and the receiver's rates swapped or put on one variant, a rate in
    /// bytes, RTT left in microseconds, the RTT `mean` dropped for one stream, either window
    /// misnamed or put on the receiver, a resolved setting dropped or added, and the local and
    /// remote CPU swapped.
    #[test]
    fn adapter_network_iperf3_tcp() {
        let results = convert("tcp.json");
        assert_eq!(results.resolved_adapter, Some(Adapter::NetworkIperf3));
        let settings = tcp_settings();
        assert_eq!(
            variant_names(&results),
            expected_variants("localhost", &settings, &[(C2S, BOTH)])
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, C2S, "sender"),
            &[
                rate(61_219_055_595.502_35),
                count(Retransmits::SLUG_STR, 0.0),
                (
                    RoundTripTime::SLUG_STR,
                    vec![("min", 13_000.0), ("max", 33_000.0), ("mean", 20_000.0)],
                ),
                (CongestionWindow::SLUG_STR, vec![("max", 1_440_626.0)]),
                (SendWindow::SLUG_STR, vec![("max", 6_192_000.0)]),
            ],
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, C2S, "receiver"),
            &[rate(61_217_852_131.412_24)],
        );
        for (cpu, host, expected) in [
            ("user", None, 0.832_830_581_272_438_6),
            ("system", None, 99.291_494_367_899_9),
            ("total", None, 100.124_391_575_618_86),
            ("user", Some("remote"), 1.577_756_475_805_213),
            ("system", Some("remote"), 50.438_695_133_121_23),
            ("total", Some("remote"), 52.016_484_927_965_9),
        ] {
            let mut extra = vec![("@cpu", cpu)];
            extra.extend(host.map(|host| ("@host", host)));
            assert_variant(
                &results,
                "localhost",
                &with(&settings, &extra),
                &[count(CpuUtilization::SLUG_STR, expected)],
            );
        }
        assert_ignores_average(Adapter::NetworkIperf3, &format!("{IPERF3}/tcp.json"));

        assert_value(
            &convert("tcp_netem_retransmits_3.16.json"),
            ("127.0.0.1", &side(&settings, C2S, "sender")),
            (Retransmits::SLUG_STR, "value"),
            504.0,
        );
    }

    /// Kills: the `-T` title ignored for the host, and the host not used without a title.
    #[test]
    fn adapter_network_iperf3_name() {
        let settings = settings(&json!({
            "@protocol": "TCP",
            "@parallel": 2,
            "@time": 3,
            "@bitrate": 0,
            "@length": 128 * 1024,
        }));
        assert_eq!(
            variant_names(&convert("tcp_title.json")),
            expected_variants("mytitle", &settings, &[(C2S, BOTH)])
        );
        let settings = settings_of(&[("@time", json!(10))]);
        let results = convert("issue_454_tcp_3.16.json");
        assert_eq!(
            variant_names(&results),
            expected_variants("...", &settings, &[(C2S, BOTH)])
        );
        assert_variant(
            &results,
            "...",
            &side(&settings, C2S, "sender"),
            &[
                rate(16_336_517.088_402_608),
                count(Retransmits::SLUG_STR, 0.0),
                (
                    RoundTripTime::SLUG_STR,
                    vec![
                        ("min", 209_594_000.0),
                        ("max", 220_392_000.0),
                        ("mean", 213_197_000.0),
                    ],
                ),
                (CongestionWindow::SLUG_STR, vec![("max", 983_628.0)]),
                (SendWindow::SLUG_STR, vec![("max", 2_174_976.0)]),
            ],
        );
        assert_variant(
            &results,
            "...",
            &side(&settings, C2S, "receiver"),
            &[rate(14_120_182.377_185_717)],
        );

        let results = convert_edited("tcp.json", |json| {
            json["title"] = "".into();
        });
        assert_eq!(results.skipped.benchmarks, 1);
        assert!(results.inner.is_empty());
    }

    /// The TCP settings with `changes` made, a `null` value removing a key.
    fn settings_of(changes: &[(&str, Value)]) -> Map<String, Value> {
        let mut settings = tcp_settings();
        for (key, value) in changes {
            if value.is_null() {
                settings.remove(*key);
            } else {
                settings.insert((*key).to_owned(), value.clone());
            }
        }
        settings
    }

    /// Kills: the RTT `min` or `max` taken from one stream, a `mean` of the streams' means
    /// invented, a `mean` written when only one of several streams was sampled, a window summed
    /// or taken from one stream, and `@parallel` dropped.
    #[test]
    fn adapter_network_iperf3_parallel() {
        let settings = settings_of(&[("@parallel", json!(4))]);
        let results = convert("tcp_parallel4.json");
        assert_eq!(
            variant_names(&results),
            expected_variants("localhost", &settings, &[(C2S, BOTH)])
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, C2S, "sender"),
            &[
                rate(205_175_497_330.280_18),
                count(Retransmits::SLUG_STR, 1.0),
                (
                    RoundTripTime::SLUG_STR,
                    vec![("min", 16_000.0), ("max", 47_000.0)],
                ),
                (CongestionWindow::SLUG_STR, vec![("max", 2_029_973.0)]),
                (SendWindow::SLUG_STR, vec![("max", 6_192_000.0)]),
            ],
        );

        // A stream iperf3 never sampled writes zeros, which are no minimum.
        let results = convert_edited("tcp_parallel4.json", |json| {
            let stream = &mut json["end"]["streams"][3]["sender"];
            for key in [
                "max_rtt",
                "min_rtt",
                "mean_rtt",
                "max_snd_cwnd",
                "max_snd_wnd",
            ] {
                stream[key] = 0.into();
            }
        });
        assert_variant(
            &results,
            "localhost",
            &side(&settings, C2S, "sender"),
            &[
                rate(205_175_497_330.280_18),
                count(Retransmits::SLUG_STR, 1.0),
                (
                    RoundTripTime::SLUG_STR,
                    vec![("min", 19_000.0), ("max", 37_000.0)],
                ),
                (CongestionWindow::SLUG_STR, vec![("max", 2_029_973.0)]),
                (SendWindow::SLUG_STR, vec![("max", 5_787_776.0)]),
            ],
        );

        // One sampled stream of four gives no `mean`: the other streams' RTTs are unknown.
        let results = convert_edited("tcp_parallel4.json", |json| {
            for index in [0, 1, 3] {
                let stream = &mut json["end"]["streams"][index]["sender"];
                for key in ["max_rtt", "min_rtt", "mean_rtt"] {
                    stream[key] = 0.into();
                }
            }
        });
        assert_variant(
            &results,
            "localhost",
            &side(&settings, C2S, "sender"),
            &[
                rate(205_175_497_330.280_18),
                count(Retransmits::SLUG_STR, 1.0),
                (
                    RoundTripTime::SLUG_STR,
                    vec![("min", 34_000.0), ("max", 35_000.0)],
                ),
                (CongestionWindow::SLUG_STR, vec![("max", 2_029_973.0)]),
                (SendWindow::SLUG_STR, vec![("max", 6_192_000.0)]),
            ],
        );
    }

    /// Kills: a reverse run's flow named `client_to_server`, and the RTT and windows of a
    /// stream where the client only received, which iperf3 writes as zeros, stored.
    #[test]
    fn adapter_network_iperf3_reverse() {
        let settings = tcp_settings();
        let results = convert("tcp_reverse.json");
        assert_eq!(
            variant_names(&results),
            expected_variants("localhost", &settings, &[(S2C, BOTH)])
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, S2C, "sender"),
            &[
                rate(55_736_829_163.429_23),
                count(Retransmits::SLUG_STR, 0.0),
            ],
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, S2C, "receiver"),
            &[rate(55_735_966_531.568_92)],
        );
    }

    /// Kills: `@bidir` dropped from any variant, the reverse flow's sums dropped or named
    /// `client_to_server`, and the RTT of the client's receiving socket put on the reverse
    /// flow.
    #[test]
    fn adapter_network_iperf3_bidir() {
        let settings = settings_of(&[("@bidir", json!(true))]);
        let results = convert("tcp_bidir.json");
        assert_eq!(
            variant_names(&results),
            expected_variants("localhost", &settings, &[(C2S, BOTH), (S2C, BOTH)])
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, C2S, "sender"),
            &[
                rate(54_999_508_176.948_074),
                count(Retransmits::SLUG_STR, 0.0),
                (
                    RoundTripTime::SLUG_STR,
                    vec![("min", 14_000.0), ("max", 38_000.0), ("mean", 22_000.0)],
                ),
                (CongestionWindow::SLUG_STR, vec![("max", 1_833_524.0)]),
                (SendWindow::SLUG_STR, vec![("max", 6_192_000.0)]),
            ],
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, C2S, "receiver"),
            &[rate(54_994_688_971.032_87)],
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, S2C, "sender"),
            &[
                rate(56_731_737_636.618_44),
                count(Retransmits::SLUG_STR, 0.0),
            ],
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, S2C, "receiver"),
            &[rate(56_715_936_795.707_5)],
        );
    }

    /// Kills: a `--bidir` run before iperf3 3.11, which repeats each sum's key, refused or read
    /// from one copy, its copies paired with the flows swapped or by their order rather than
    /// their `sender` value, TCP's unlimited 0 not written for it, and a repeated key whose
    /// copies do not hold one flow each read or dropped.
    #[test]
    fn adapter_network_iperf3_bidir_before_3_11() {
        let settings = settings_of(&[("@bidir", json!(true))]);
        let results = convert("tcp_bidir_3.10.json");
        assert_eq!(
            variant_names(&results),
            expected_variants("localhost", &settings, &[(C2S, BOTH), (S2C, BOTH)])
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, C2S, "sender"),
            &[
                rate(61_307_050_753.360_825),
                count(Retransmits::SLUG_STR, 0.0),
                (
                    RoundTripTime::SLUG_STR,
                    vec![("min", 19_000.0), ("max", 34_000.0), ("mean", 24_000.0)],
                ),
                (CongestionWindow::SLUG_STR, vec![("max", 2_946_735.0)]),
                (SendWindow::SLUG_STR, vec![("max", 6_142_976.0)]),
            ],
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, C2S, "receiver"),
            &[rate(61_306_192_511.294_25)],
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, S2C, "sender"),
            &[
                rate(6_140_840_782.945_953),
                count(Retransmits::SLUG_STR, 0.0),
            ],
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, S2C, "receiver"),
            &[rate(6_130_881_377.829_063)],
        );
        assert_value(
            &results,
            ("localhost", &with(&settings, &[("@cpu", "total")])),
            (CpuUtilization::SLUG_STR, "value"),
            99.987_904_991_206_96,
        );

        // The server's sums written first, then the client's.
        let input = read("tcp_bidir_3.10.json");
        let sums: Vec<usize> = input
            .match_indices("\t\t\"sum_sent\":")
            .map(|(index, _)| index)
            .collect();
        let [client, server] = sums[..] else {
            panic!("Failed to find the two sum_sent copies");
        };
        let cpu = input
            .find("\t\t\"cpu_utilization_percent\":")
            .expect("Failed to find the CPU");
        let server_first: String = [0..client, server..cpu, client..server, cpu..input.len()]
            .into_iter()
            .map(|range| input.get(range).expect("Failed to slice the report"))
            .collect();
        let client_first = parse(&input);
        assert!(client_first.is_some());
        assert_eq!(parse(&server_first), client_first);

        for key in ["\t\t\"sum_sent\":", "\t\t\"sum_received\":"] {
            let (second, _) = input
                .match_indices(key)
                .nth(1)
                .expect("Failed to find the second copy");
            let (head, tail) = input.split_at(second);
            let both_sender = format!(
                "{head}{}",
                tail.replacen("\"sender\":\tfalse\n\t\t},", "\"sender\":\ttrue\n\t\t},", 1)
            );
            assert_ne!(both_sender, input);
            assert!(parse(&both_sender).is_none(), "{key}");
        }
    }

    /// Kills: a UDP `--bidir` run before iperf3 3.11, which repeats `sum`, refused, its flows
    /// swapped, and the server's flow's losses read from the client's.
    #[test]
    fn adapter_network_iperf3_udp_bidir_before_3_11() {
        let settings = udp_settings(&[("@bitrate", Value::Null), ("@bidir", json!(true))]);
        let results = convert("udp_bidir_3.10.json");
        assert_eq!(
            variant_names(&results),
            expected_variants("localhost", &settings, &[(C2S, BOTH), (S2C, BOTH)])
        );
        for direction in [C2S, S2C] {
            assert_variant(
                &results,
                "localhost",
                &side(&settings, direction, "sender"),
                &[rate(100_049_892.468_530_54)],
            );
        }
        assert_variant(
            &results,
            "localhost",
            &side(&settings, C2S, "receiver"),
            &losses(0.006_023_540_259_679_53, 0.0, 0.0),
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, S2C, "receiver"),
            &[
                &[count(OutOfOrderPackets::SLUG_STR, 0.0)][..],
                &losses(0.005_646_752_897_181_358, 0.0, 0.0),
            ]
            .concat(),
        );
    }

    /// Kills: `@time` written beside `@bytes` or `@blocks`, `@bytes` or `@blocks` dropped,
    /// `@omit` dropped when set or written when 0, a TCP rate limit before iperf3 3.11, which
    /// only `start` holds, dropped or read before the test's own rate, TCP's unlimited 0 dropped
    /// from 3.7 to 3.10, which splits the series at 3.11, and a `@bitrate` invented for SCTP
    /// before 3.11.
    #[test]
    fn adapter_network_iperf3_settings() {
        for (fixture, settings) in [
            (
                "tcp_bytes.json",
                settings_of(&[("@time", Value::Null), ("@bytes", json!(104_857_600))]),
            ),
            (
                "tcp_blocks.json",
                settings_of(&[("@time", Value::Null), ("@blocks", json!(2000))]),
            ),
            ("tcp_omit.json", settings_of(&[("@omit", json!(1))])),
            ("tcp_3.10.json", tcp_settings()),
            (
                "tcp_bitrate_3.10.json",
                settings_of(&[("@bitrate", json!(1_000_000_000))]),
            ),
            ("tcp_3.16.json", tcp_settings()),
            ("tcp_interval0.json", tcp_settings()),
            ("tcp_server_output.json", tcp_settings()),
        ] {
            assert_eq!(
                variant_names(&convert(fixture)),
                expected_variants("localhost", &settings, &[(C2S, BOTH)]),
                "{fixture}"
            );
        }
        let results = convert("tcp_3.10.json");
        assert_value(
            &results,
            ("localhost", &side(&tcp_settings(), C2S, "sender")),
            (RoundTripTime::SLUG_STR, "mean"),
            17_000.0,
        );
        assert_value(
            &convert("tcp_bitrate_3.10.json"),
            (
                "localhost",
                &side(
                    &settings_of(&[("@bitrate", json!(1_000_000_000))]),
                    C2S,
                    "receiver",
                ),
            ),
            (BitsThroughput::SLUG_STR, "mean"),
            999_960_313.256_747,
        );

        // From 3.11, the test's own rate wins over the one `start` holds.
        let results = convert_edited("tcp.json", |json| {
            json["start"]["target_bitrate"] = 1_000_000_000.into();
        });
        assert_eq!(
            variant_names(&results),
            expected_variants("localhost", &tcp_settings(), &[(C2S, BOTH)])
        );

        let sctp = settings_of(&[("@protocol", json!("SCTP")), ("@bitrate", Value::Null)]);
        let results = convert_edited("tcp_3.10.json", |json| {
            json["start"]["test_start"]["protocol"] = "SCTP".into();
        });
        assert_eq!(
            variant_names(&results),
            expected_variants("localhost", &sctp, &[(C2S, BOTH)])
        );
    }

    fn udp_settings(changes: &[(&str, Value)]) -> Map<String, Value> {
        let mut settings = settings(&json!({
            "@protocol": "UDP",
            "@parallel": 1,
            "@time": 3,
            "@bitrate": 100_000_000,
            "@length": 32 * 1024,
        }));
        for (key, value) in changes {
            if value.is_null() {
                settings.remove(*key);
            } else {
                settings.insert((*key).to_owned(), value.clone());
            }
        }
        settings
    }

    fn losses(jitter_ms: f64, lost: f64, percent: f64) -> [Expected; 3] {
        [
            (PacketJitter::SLUG_STR, vec![("value", jitter_ms * 1e6)]),
            count(LostPackets::SLUG_STR, lost),
            (PacketLoss::SLUG_STR, vec![("value", percent)]),
        ]
    }

    /// Kills: `@length` dropped for UDP or not read from the resolved datagram size, the jitter
    /// left in milliseconds or read from the sender's sum, where iperf3 writes 0, the loss put on
    /// the sender, and an out-of-order count written for a flow the client sent, which only the
    /// server counts.
    #[test]
    fn adapter_network_iperf3_udp() {
        let settings = udp_settings(&[]);
        let results = convert("udp.json");
        assert_eq!(
            variant_names(&results),
            expected_variants("localhost", &settings, &[(C2S, BOTH)])
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, C2S, "sender"),
            &[rate(100_006_390.442_723_08)],
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, C2S, "receiver"),
            &[
                &[rate(100_003_991.432_081_2)][..],
                &losses(0.006_777_551_902_110_85, 0.0, 0.0),
            ]
            .concat(),
        );

        let settings = udp_settings(&[("@bitrate", json!(200_000_000))]);
        let results = convert("udp_netem_loss_3.16.json");
        assert_variant(
            &results,
            "127.0.0.1",
            &side(&settings, C2S, "receiver"),
            &[
                &[rate(196_674_337.179_255_16)][..],
                &losses(0.009_035_900_825_679_39, 38.0, 1.660_113_586_719_091_3),
            ]
            .concat(),
        );

        let settings = udp_settings(&[
            ("@bitrate", json!(20_000_000_000u64)),
            ("@length", json!(1400)),
        ]);
        let results = convert("udp_loss_reverse.json");
        assert_eq!(
            variant_names(&results),
            expected_variants("localhost", &settings, &[(S2C, BOTH)])
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, S2C, "sender"),
            &[rate(5_433_494_838.789_433)],
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, S2C, "receiver"),
            &[
                &[
                    rate(5_427_778_144.316_155),
                    count(OutOfOrderPackets::SLUG_STR, 0.0),
                ][..],
                &losses(
                    0.002_345_879_005_849_541_7,
                    1523.0,
                    0.104_605_096_871_325_42,
                ),
            ]
            .concat(),
        );
        assert!(convert("udp_loss.json").has_metrics());
        assert!(convert("udp_3.16.json").has_metrics());
    }

    /// Kills: a UDP `--bidir` run's direction taken from its sums' `sender` field, which
    /// iperf3 fixes to `true` for every `sum_sent`, and the reverse flow's losses read from the
    /// forward flow.
    #[test]
    fn adapter_network_iperf3_udp_bidir() {
        let settings = udp_settings(&[("@bidir", json!(true))]);
        let results = convert("udp_bidir.json");
        assert_eq!(
            variant_names(&results),
            expected_variants("localhost", &settings, &[(C2S, BOTH), (S2C, BOTH)])
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, C2S, "receiver"),
            &[
                &[rate(100_008_056.490_055_83)][..],
                &losses(0.005_572_637_104_773_913, 0.0, 0.0),
            ]
            .concat(),
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, S2C, "receiver"),
            &[
                &[
                    rate(100_008_056.490_055_83),
                    count(OutOfOrderPackets::SLUG_STR, 0.0),
                ][..],
                &losses(0.006_541_879_896_539_722, 0.0, 0.0),
            ]
            .concat(),
        );
        assert!(convert("udp_parallel2.json").has_metrics());
    }

    /// Kills: a UDP run before iperf3 3.11, which writes only `sum`, refused, a receiver rate
    /// invented for it, its losses dropped, and the out-of-order counts of a flow's streams
    /// taken from one stream rather than summed.
    #[test]
    fn adapter_network_iperf3_udp_before_3_11() {
        let settings = udp_settings(&[("@bitrate", Value::Null)]);
        let results = convert("udp_3.10.json");
        assert_eq!(
            variant_names(&results),
            expected_variants("localhost", &settings, &[(C2S, BOTH)])
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, C2S, "sender"),
            &[rate(100_049_225.485_255_03)],
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, C2S, "receiver"),
            &losses(0.009_036_212_905_127_01, 0.0, 0.0),
        );

        let settings = udp_settings(&[("@bitrate", Value::Null), ("@parallel", json!(2))]);
        let results = convert_edited("udp_parallel2_reverse_3.10.json", |json| {
            json["end"]["streams"][0]["udp"]["out_of_order"] = 3.into();
            json["end"]["streams"][1]["udp"]["out_of_order"] = 4.into();
        });
        assert_variant(
            &results,
            "localhost",
            &side(&settings, S2C, "sender"),
            &[rate(200_091_447.937_905)],
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, S2C, "receiver"),
            &[
                &[count(OutOfOrderPackets::SLUG_STR, 7.0)][..],
                &losses(0.038_438_201_696_801_24, 0.0, 0.0),
            ]
            .concat(),
        );
    }

    /// Kills: zeros stored as a measured RTT or window where iperf3 took no sample, a platform
    /// with no TCP state (no `max_rtt`, no `retransmits`) refused, SCTP's RTT, in milliseconds,
    /// read as TCP's microseconds, and `@length` dropped for SCTP.
    #[test]
    fn adapter_network_iperf3_no_tcp_state() {
        let settings = tcp_settings();
        let sender = side(&settings, C2S, "sender");
        let results = convert_edited("tcp.json", |json| {
            let stream = &mut json["end"]["streams"][0]["sender"];
            for key in [
                "max_rtt",
                "min_rtt",
                "mean_rtt",
                "max_snd_cwnd",
                "max_snd_wnd",
            ] {
                stream[key] = 0.into();
            }
        });
        assert_variant(
            &results,
            "localhost",
            &sender,
            &[
                rate(61_219_055_595.502_35),
                count(Retransmits::SLUG_STR, 0.0),
            ],
        );

        let results = convert_edited("tcp.json", |json| {
            let stream = json["end"]["streams"][0]["sender"]
                .as_object_mut()
                .expect("stream");
            for key in [
                "retransmits",
                "reorder",
                "max_rtt",
                "min_rtt",
                "mean_rtt",
                "max_snd_cwnd",
                "max_snd_wnd",
            ] {
                stream.remove(key);
            }
            json["end"]["sum_sent"]
                .as_object_mut()
                .expect("sum_sent")
                .remove("retransmits");
        });
        assert_variant(
            &results,
            "localhost",
            &sender,
            &[rate(61_219_055_595.502_35)],
        );

        let sctp = settings_of(&[("@protocol", json!("SCTP"))]);
        let results = convert_edited("tcp.json", |json| {
            json["start"]["test_start"]["protocol"] = "SCTP".into();
        });
        assert_variant(
            &results,
            "localhost",
            &side(&sctp, C2S, "sender"),
            &[
                rate(61_219_055_595.502_35),
                count(Retransmits::SLUG_STR, 0.0),
            ],
        );
    }

    /// Kills: a stream with no `sender`, as iperf3 before 3.7 writes it, refused or put on the
    /// wrong side, and a TCP rate of 0 written before 3.7, which records no `-b` rate.
    #[test]
    fn adapter_network_iperf3_before_3_7() {
        let settings = settings_of(&[("@bitrate", Value::Null)]);
        let results = convert("tcp_bitrate_3.6.json");
        assert_eq!(
            variant_names(&results),
            expected_variants("localhost", &settings, &[(C2S, BOTH)])
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, C2S, "sender"),
            &[
                rate(999_961_700.666_792_4),
                count(Retransmits::SLUG_STR, 0.0),
                (
                    RoundTripTime::SLUG_STR,
                    vec![("min", 44_000.0), ("max", 79_000.0), ("mean", 64_000.0)],
                ),
                (CongestionWindow::SLUG_STR, vec![("max", 1_375_143.0)]),
            ],
        );
        assert_variant(
            &results,
            "localhost",
            &side(&settings, C2S, "receiver"),
            &[rate(987_803_126.638_022_4)],
        );

        let udp = udp_settings(&[
            ("@bitrate", json!(20_000_000_000u64)),
            ("@length", json!(1400)),
        ]);
        let results = convert_edited("udp_loss_reverse.json", |json| {
            json["end"]["streams"][0]["udp"]
                .as_object_mut()
                .expect("udp")
                .remove("sender");
        });
        assert_value(
            &results,
            ("localhost", &side(&udp, S2C, "receiver")),
            (OutOfOrderPackets::SLUG_STR, "value"),
            0.0,
        );
    }

    /// Kills: a run iperf3 ended in an error read, whether its `end` is empty (refused, server
    /// gone) or holds the partial sums of an interrupted run, `--json-stream` lines read, a
    /// server's report (no `connecting_to`) read, also when the client's `-T` title names it,
    /// and JSON that is not iperf3's claimed.
    #[test]
    fn adapter_network_iperf3_refusals() {
        for fixture in [
            "refused.json",
            "server_killed_3.16.json",
            "tcp_sigint.json",
            "server_title_3.22.json",
            "tcp_json_stream.txt",
            "tcp_text.txt",
        ] {
            assert!(parse(&read(fixture)).is_none(), "{fixture}");
        }
        let input = read("tcp.json");
        assert!(parse(&input).is_some());
        assert!(parse(&input.replace("\"iperf 3.22\"", "\"iperf 2.1.9\"")).is_none());
        let mut json: Value = serde_json::from_str(&input).expect("Failed to parse");
        json["start"]
            .as_object_mut()
            .expect("start")
            .remove("connecting_to");
        assert!(parse(&json.to_string()).is_none());
    }
}
