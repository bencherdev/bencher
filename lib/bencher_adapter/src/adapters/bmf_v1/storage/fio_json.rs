use std::collections::BTreeMap;

use bencher_json::{
    MeasureNameId,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{BytesThroughput, Latency, Throughput},
            systems::{
                CompletionLatency, ContextSwitches, IoErrors, MajorPageFaults, MinorPageFaults,
                SubmissionLatency,
            },
        },
        report::Adapter,
    },
};
use serde::Deserialize;

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::storage::fio::{Cpu, Direction, Job, Metrics, Outcome, Run, results},
    results::adapter_results::AdapterResults,
};

/// The options a fio job's parameters hold, each only when the user set it.
const WORKLOAD: [&str; 12] = [
    "rw",
    "bs",
    "iodepth",
    "numjobs",
    "ioengine",
    "direct",
    "rwmixread",
    "size",
    "runtime",
    "time_based",
    "fsync",
    "buffered",
];

const KIB: f64 = 1024.0;

/// fio's JSON, `json` or `json+`, as fio 3.0 and later write it, with its latencies in
/// nanoseconds.
///
/// The `note:` and `fio:` lines fio prints to stdout before its JSON are skipped, and of the
/// reports `--status-interval` writes, the last, fio's final cumulative one, is read.
pub struct AdapterStorageFioJson;

impl Adaptable for AdapterStorageFioJson {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut reports = serde_json::Deserializer::from_str(json(input)?).into_iter::<Report>();
        let mut report = reports.next()?.ok()?;
        for next in reports {
            report = next.ok()?;
        }
        if !report.version.starts_with("fio-") || report.jobs.iter().all(JsonJob::idle) {
            return None;
        }
        let jobs: Vec<Job> = report
            .jobs
            .iter()
            .map(|job| job.job(&report.global_options))
            .collect();
        results(&jobs, 0, Adapter::StorageFioJson)
    }
}

/// The input from its first line that opens a JSON object, past the lines fio prints before it.
fn json(input: &str) -> Option<&str> {
    let mut rest = input;
    while !rest.starts_with('{') {
        let (line, next) = rest.split_once('\n')?;
        if !(line.starts_with("note:") || line.starts_with("fio:")) {
            return None;
        }
        rest = next;
    }
    Some(rest)
}

type Options = serde_json::Map<String, serde_json::Value>;

/// A report, whose `global options` hold a job file's `[global]` section. Client/server output
/// holds `client_stats` in place of `jobs`.
#[derive(Deserialize)]
struct Report {
    #[serde(rename = "fio version")]
    version: String,
    #[serde(rename = "global options", default)]
    global_options: Options,
    jobs: Vec<JsonJob>,
}

/// A job, with `total_err` only under `continue_on_error`, and under `unified_rw_reporting` a
/// `mixed` direction, in place of the others or beside them.
#[derive(Deserialize)]
struct JsonJob {
    jobname: String,
    error: i64,
    #[serde(rename = "job options", default)]
    options: Options,
    read: Option<IoDirection>,
    write: Option<IoDirection>,
    trim: Option<IoDirection>,
    mixed: Option<IoDirection>,
    sync: Option<SyncDirection>,
    usr_cpu: f64,
    sys_cpu: f64,
    ctx: f64,
    majf: f64,
    minf: f64,
    total_err: Option<f64>,
}

impl JsonJob {
    /// The job, failed when it ended in an error that `continue_on_error` did not count or before
    /// any I/O, and idle when it issued no I/O, as fio's text then prints nothing of it
    /// (`stat.c:1239`).
    fn job(&self, global_options: &Options) -> Job {
        let outcome = if self.error != 0 && (self.total_err.is_none() || self.idle()) {
            Outcome::Failed
        } else if self.idle() {
            Outcome::Idle
        } else {
            Outcome::Ran(self.run(global_options))
        };
        Job {
            name: self.jobname.clone(),
            outcome,
        }
    }

    /// The job's results, with a direction only when it moved data, as fio's text prints it
    /// (`stat.c:1262`), and `sync` only when it has samples (`stat.c:1271`).
    fn run(&self, global_options: &Options) -> Run {
        let mut options = Options::new();
        for (key, value) in global_options.iter().chain(&self.options) {
            if WORKLOAD.contains(&key.as_str()) {
                options.insert(format!("@{key}"), value.clone());
            }
        }
        let mut metrics = Metrics::default();
        for (measure, value) in [
            (ContextSwitches::name_id(), self.ctx),
            (MajorPageFaults::name_id(), self.majf),
            (MinorPageFaults::name_id(), self.minf),
        ] {
            metrics.insert(&measure, "value", value);
        }
        if let Some(total_err) = self.total_err {
            metrics.insert(&IoErrors::name_id(), "value", total_err);
        }
        let mut active = Vec::new();
        for (direction, stats) in self.directions() {
            if let Some(stats) = stats
                && stats.io_bytes > 0.0
            {
                active.push((direction, stats.metrics()));
            }
        }
        if let Some(SyncDirection { lat_ns }) = &self.sync
            && lat_ns.has_samples()
        {
            let mut metrics = Metrics::default();
            lat_ns.insert(&mut metrics, &Latency::name_id());
            active.push((Direction::Sync, metrics));
        }
        Run {
            options,
            metrics,
            directions: active,
            cpu: Some(Cpu {
                user: self.usr_cpu,
                system: self.sys_cpu,
            }),
        }
    }

    /// Whether the job issued no I/O, failed or not.
    fn idle(&self) -> bool {
        self.directions().iter().all(|(_, stats)| {
            stats
                .as_ref()
                .is_none_or(|stats| stats.io_bytes <= 0.0 && stats.total_ios <= 0.0)
        })
    }

    fn directions(&self) -> [(Direction, &Option<IoDirection>); 4] {
        [
            (Direction::Read, &self.read),
            (Direction::Write, &self.write),
            (Direction::Trim, &self.trim),
            (Direction::Mixed, &self.mixed),
        ]
    }
}

/// A direction that moves data, with `bw_bytes` from fio 3.2, and the samples' bandwidth in
/// KiB/s.
#[derive(Deserialize)]
struct IoDirection {
    io_bytes: f64,
    bw_bytes: Option<f64>,
    bw: f64,
    iops: f64,
    total_ios: f64,
    slat_ns: LatencyStats,
    clat_ns: LatencyStats,
    lat_ns: LatencyStats,
    bw_min: f64,
    bw_max: f64,
    bw_mean: f64,
    bw_dev: f64,
    bw_samples: f64,
    iops_min: f64,
    iops_max: f64,
    iops_mean: f64,
    iops_stddev: f64,
    iops_samples: f64,
}

impl IoDirection {
    /// The direction's latencies, its run rates as `mean`, and its rates over fio's sample
    /// windows when it took any, with the run's bandwidth from `bw` in KiB/s before fio 3.2.
    fn metrics(&self) -> Metrics {
        let mut metrics = Metrics::default();
        for (stats, measure) in [
            (&self.lat_ns, Latency::name_id()),
            (&self.clat_ns, CompletionLatency::name_id()),
            (&self.slat_ns, SubmissionLatency::name_id()),
        ] {
            if stats.has_samples() {
                stats.insert(&mut metrics, &measure);
            }
        }
        for (measure, mean, samples, windows, factor) in [
            (
                Throughput::name_id(),
                self.iops,
                self.iops_samples,
                [
                    self.iops_min,
                    self.iops_max,
                    self.iops_mean,
                    self.iops_stddev,
                ],
                1.0,
            ),
            (
                BytesThroughput::name_id(),
                self.bw_bytes.unwrap_or(self.bw * KIB),
                self.bw_samples,
                [self.bw_min, self.bw_max, self.bw_mean, self.bw_dev],
                KIB,
            ),
        ] {
            metrics.insert(&measure, "mean", mean);
            if samples > 0.0 {
                for (metric, value) in ["window_min", "window_max", "window_mean", "window_std_dev"]
                    .into_iter()
                    .zip(windows)
                {
                    metrics.insert(&measure, metric, value * factor);
                }
            }
        }
        metrics
    }
}

/// The `sync` direction, from fio 3.4.
#[derive(Deserialize)]
struct SyncDirection {
    lat_ns: LatencyStats,
}

/// A latency in nanoseconds, with `N` from fio 3.18, and its percentiles, fio's bin estimates,
/// when it reports them.
#[derive(Deserialize)]
struct LatencyStats {
    min: f64,
    max: f64,
    mean: f64,
    stddev: f64,
    #[serde(rename = "N")]
    samples: Option<f64>,
    #[serde(default)]
    percentile: BTreeMap<String, f64>,
}

impl LatencyStats {
    /// Whether the latency has samples, or, before fio wrote `N`, a maximum above the zeros fio
    /// writes for a latency with none (`stat.c:1467`).
    fn has_samples(&self) -> bool {
        self.samples.map_or(self.max > 0.0, |samples| samples > 0.0)
    }

    fn insert(&self, metrics: &mut Metrics, measure: &MeasureNameId) {
        metrics.insert_summary(measure, [self.min, self.max, self.mean, self.stddev]);
        for (percent, value) in &self.percentile {
            metrics.insert_percentile(measure, percent, *value);
        }
    }
}

#[cfg(test)]
mod test_storage_fio_json {
    use std::collections::BTreeSet;

    use bencher_json::{
        BenchmarkName, BenchmarkNameId, BmfVersion, MeasureNameId, ParameterSet,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{BytesThroughput, Latency, Throughput},
                systems::{
                    CompletionLatency, ContextSwitches, CpuUtilization, IoErrors, MajorPageFaults,
                    MinorPageFaults, SubmissionLatency,
                },
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            assert_ignores_average, assert_value, assert_variant, convert_edited_v1, convert_v1,
            value, variants,
        },
        results::{adapter_metrics::AdapterMetrics, adapter_results::AdapterResults},
    };

    const FIO: &str = "storage/fio";
    const RANDREAD: &str = r#"{"@bs":"4k","@ioengine":"psync","@runtime":"2","@rw":"randread","@size":"32M","@time_based":""}"#;

    fn convert(fixture: &str) -> AdapterResults {
        convert_v1(Adapter::StorageFioJson, &format!("{FIO}/{fixture}"))
    }

    fn convert_edited<F: FnOnce(&mut serde_json::Value)>(fixture: &str, edit: F) -> AdapterResults {
        convert_edited_v1(Adapter::StorageFioJson, &format!("{FIO}/{fixture}"), edit)
    }

    fn parse(input: &str) -> Option<AdapterResults> {
        Adapter::StorageFioJson.convert(input, Settings::new(None, BmfVersion::V1))
    }

    fn read(fixture: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/{FIO}/{fixture}"))
            .expect("Failed to read a fixture")
    }

    /// The parameters with `extra` added.
    fn with(parameters: &str, extra: &[(&str, serde_json::Value)]) -> String {
        let mut parameters: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(parameters).expect("Failed to parse parameters");
        for (key, value) in extra {
            parameters.insert((*key).to_owned(), value.clone());
        }
        serde_json::Value::Object(parameters).to_string()
    }

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

    fn expected_names(expected: &[(&str, String)]) -> BTreeSet<(String, String)> {
        expected
            .iter()
            .map(|(name, parameters)| {
                let parameters: ParameterSet =
                    parameters.parse().expect("Failed to parse parameters");
                ((*name).to_owned(), parameters.canonical())
            })
            .collect()
    }

    /// The variants of one job: its own, one per direction that did I/O, and one per CPU mode.
    fn job_variants(name: &str, parameters: &str, directions: &[&str]) -> Vec<(String, String)> {
        let mut variants = vec![
            (name.to_owned(), parameters.to_owned()),
            (
                name.to_owned(),
                with(parameters, &[("@cpu", "user".into())]),
            ),
            (
                name.to_owned(),
                with(parameters, &[("@cpu", "system".into())]),
            ),
        ];
        for direction in directions {
            variants.push((
                name.to_owned(),
                with(parameters, &[("@direction", (*direction).into())]),
            ));
        }
        variants
    }

    fn measure_names(results: &AdapterResults, name: &str, parameters: &str) -> BTreeSet<String> {
        metrics(results, name, parameters)
            .inner
            .keys()
            .map(ToString::to_string)
            .collect()
    }

    fn metric_names(
        results: &AdapterResults,
        name: &str,
        parameters: &str,
        measure: &str,
    ) -> BTreeSet<String> {
        let measure: MeasureNameId = measure.parse().expect("Failed to parse measure");
        metrics(results, name, parameters).inner[&measure]
            .inner
            .keys()
            .map(ToString::to_string)
            .collect()
    }

    fn metrics<'a>(
        results: &'a AdapterResults,
        name: &str,
        parameters: &str,
    ) -> &'a AdapterMetrics {
        let name: BenchmarkName = name.parse().expect("Failed to parse benchmark");
        let parameters: ParameterSet = parameters.parse().expect("Failed to parse parameters");
        &results.inner[&BenchmarkNameId::new_name(name)][&parameters]
    }

    fn assert_variants(results: &AdapterResults, expected: &[(String, String)]) {
        let expected: Vec<(&str, String)> = expected
            .iter()
            .map(|(name, parameters)| (name.as_str(), parameters.clone()))
            .collect();
        assert_eq!(variant_names(results), expected_names(&expected));
    }

    /// Kills: an idle direction (no I/O, or `sync` with no samples) written, `@direction` or
    /// `@cpu` dropped, a per job quantity put on a direction, and a path, `name`, or an
    /// unlisted option made a parameter.
    #[test]
    fn adapter_storage_fio_json_variants() {
        let results = convert("randread.json");
        assert_eq!(results.resolved_adapter, Some(Adapter::StorageFioJson));
        assert_variants(&results, &job_variants("randread", RANDREAD, &["read"]));
        assert_eq!(
            (results.skipped.benchmarks, results.skipped.variants),
            (0, 0)
        );
    }

    /// Kills: a statistic dropped, misnamed, or scaled, `N` written, a submission latency with
    /// no samples written, the run rate taken from the samples' mean, the samples' bandwidth
    /// read in bytes rather than KiB, and a per job count misnamed.
    #[test]
    fn adapter_storage_fio_json_statistics() {
        let results = convert("randread.json");
        let kib = 1024.0;
        assert_variant(
            &results,
            "randread",
            &with(RANDREAD, &[("@direction", "read".into())]),
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("min", 12229.0),
                        ("max", 283_577.0),
                        ("mean", 21_945.431_098),
                        ("std_dev", 4_480.094_766),
                    ],
                ),
                (
                    CompletionLatency::SLUG_STR,
                    vec![
                        ("min", 12199.0),
                        ("max", 283_557.0),
                        ("mean", 21_918.875_62),
                        ("std_dev", 4_478.212_546),
                        ("p1", 13632.0),
                        ("p5", 14656.0),
                        ("p10", 15936.0),
                        ("p20", 19328.0),
                        ("p30", 20096.0),
                        ("p40", 20608.0),
                        ("median", 21376.0),
                        ("p60", 22912.0),
                        ("p70", 24448.0),
                        ("p80", 25216.0),
                        ("p90", 26496.0),
                        ("p95", 27520.0),
                        ("p99", 31360.0),
                        ("p99_5", 34560.0),
                        ("p99_9", 51968.0),
                        ("p99_95", 64768.0),
                        ("p99_99", 101_888.0),
                    ],
                ),
                (
                    Throughput::SLUG_STR,
                    vec![
                        ("mean", 44_932.533_733),
                        ("window_min", 42750.0),
                        ("window_max", 47418.0),
                        ("window_mean", 44_933.5),
                        ("window_std_dev", 2_437.438_204),
                    ],
                ),
                (
                    BytesThroughput::SLUG_STR,
                    vec![
                        ("mean", 184_043_658.0),
                        ("window_min", 171_000.0 * kib),
                        ("window_max", 189_664.0 * kib),
                        ("window_mean", 179_732.0 * kib),
                        ("window_std_dev", 9_747.035_105 * kib),
                    ],
                ),
            ],
        );
        assert_variant(
            &results,
            "randread",
            RANDREAD,
            &[
                (ContextSwitches::SLUG_STR, vec![("value", 89912.0)]),
                (MajorPageFaults::SLUG_STR, vec![("value", 0.0)]),
                (MinorPageFaults::SLUG_STR, vec![("value", 5.0)]),
            ],
        );
        for (cpu, expected) in [("user", 1.75), ("system", 21.75)] {
            assert_variant(
                &results,
                "randread",
                &with(RANDREAD, &[("@cpu", cpu.into())]),
                &[(CpuUtilization::SLUG_STR, vec![("value", expected)])],
            );
        }
        assert_ignores_average(Adapter::StorageFioJson, &format!("{FIO}/randread.json"));
    }

    /// Kills: `@job` dropped from jobs that repeat a name, numbered out of fio's order, put on a
    /// job whose name is unique, or renumbered past a failed job, and `@numjobs` dropped.
    #[test]
    fn adapter_storage_fio_json_jobs() {
        let parameters = r#"{"@bs":"4k","@ioengine":"psync","@numjobs":"4","@runtime":"2","@rw":"randwrite","@size":"32M","@time_based":""}"#;
        let results = convert("numjobs4.json");
        let expected: Vec<(String, String)> = (0..4)
            .flat_map(|job| {
                job_variants("w", &with(parameters, &[("@job", job.into())]), &["write"])
            })
            .collect();
        assert_variants(&results, &expected);
        for (job, iops) in [
            (0, 409_498.0),
            (1, 420_207.669_323),
            (2, 429_007.980_05),
            (3, 402_405.5),
        ] {
            assert_value(
                &results,
                (
                    "w",
                    &with(
                        parameters,
                        &[("@job", job.into()), ("@direction", "write".into())],
                    ),
                ),
                (Throughput::SLUG_STR, "mean"),
                iops,
            );
        }

        let results = convert("numjobs4_group.json");
        assert_variants(&results, &job_variants("w", parameters, &["write"]));

        let results = convert_edited("numjobs4.json", |json| {
            json["jobs"][1]["error"] = 5.into();
        });
        assert_eq!(results.skipped.benchmarks, 1);
        let expected: Vec<(String, String)> = [0, 2, 3]
            .into_iter()
            .flat_map(|job| {
                job_variants("w", &with(parameters, &[("@job", job.into())]), &["write"])
            })
            .collect();
        assert_variants(&results, &expected);
    }

    /// Kills: a failed job with no I/O left uncounted, with or without `continue_on_error`,
    /// `@job` renumbered past a job with no I/O, whether it failed or sat idle, an idle job
    /// counted, and a run whose only job failed before any I/O claimed.
    #[test]
    fn adapter_storage_fio_json_jobs_without_io() {
        let results = convert("open_fail.json");
        assert_eq!(results.skipped.benchmarks, 1);
        assert_variants(
            &results,
            &job_variants(
                "good",
                r#"{"@bs":"4k","@ioengine":"psync","@runtime":"2","@rw":"randread","@size":"32M","@time_based":""}"#,
                &["read"],
            ),
        );

        let results = convert("continue_on_error_open_fail.json");
        assert_eq!(results.skipped.benchmarks, 1);
        assert_variants(
            &results,
            &job_variants(
                "good",
                r#"{"@bs":"4k","@ioengine":"psync","@runtime":"1","@rw":"randread","@size":"4M","@time_based":""}"#,
                &["read"],
            ),
        );

        let parameters = r#"{"@bs":"4k","@ioengine":"psync","@numjobs":"4","@runtime":"2","@rw":"randwrite","@size":"32M","@time_based":""}"#;
        let expected: Vec<(String, String)> = [0, 1, 3]
            .into_iter()
            .flat_map(|job| {
                job_variants("w", &with(parameters, &[("@job", job.into())]), &["write"])
            })
            .collect();
        let results = convert("numjobs4_open_fail.json");
        assert_eq!(results.skipped.benchmarks, 1);
        assert_variants(&results, &expected);
        assert_value(
            &results,
            (
                "w",
                &with(
                    parameters,
                    &[("@job", 3.into()), ("@direction", "write".into())],
                ),
            ),
            (Throughput::SLUG_STR, "mean"),
            302_197.906_281,
        );

        let idle = read("numjobs4_open_fail.json").replacen(r#""error" : 13"#, r#""error" : 0"#, 1);
        let results = parse(&idle).expect("Failed to parse the idle job");
        assert_eq!(results.skipped.benchmarks, 0);
        assert_variants(&results, &expected);

        let input = read("open_fail.json");
        let (_, json) = input.split_once('\n').expect("Failed to find the JSON");
        let mut json: serde_json::Value = serde_json::from_str(json).expect("Failed to parse");
        json["jobs"].as_array_mut().expect("jobs").remove(0);
        assert!(parse(&json.to_string()).is_none());
    }

    /// Kills: `global options` ignored, a job's own option not winning over the global one, an
    /// option the user did not set made a parameter, an unlisted option (`stonewall`,
    /// `directory`, `description`) kept, and the values of one job's options leaking into
    /// another's.
    #[test]
    fn adapter_storage_fio_json_options() {
        let global = r#"{"@direct":"1","@ioengine":"io_uring","@runtime":"2","@size":"32M","@time_based":""}"#;
        let seq_read = with(
            global,
            &[
                ("@bs", "128k".into()),
                ("@iodepth", "16".into()),
                ("@rw", "read".into()),
            ],
        );
        let rand_write = with(
            global,
            &[
                ("@bs", "4k".into()),
                ("@iodepth", "32".into()),
                ("@rw", "randwrite".into()),
            ],
        );
        let mut expected = job_variants("seq-read", &seq_read, &["read"]);
        expected.extend(job_variants("rand-write", &rand_write, &["write"]));
        assert_variants(&convert("jobfile_two_jobs.json"), &expected);

        let results = convert_edited("jobfile_two_jobs.json", |json| {
            json["global options"]["bs"] = "64k".into();
            json["global options"]["rwmixread"] = "70".into();
        });
        let edited = with(&seq_read, &[("@rwmixread", "70".into())]);
        assert_variants(
            &results,
            &[
                job_variants("seq-read", &edited, &["read"]),
                job_variants(
                    "rand-write",
                    &with(&rand_write, &[("@rwmixread", "70".into())]),
                    &["write"],
                ),
            ]
            .concat(),
        );

        let defaults =
            r#"{"@bs":"4k","@runtime":"2","@rw":"randread","@size":"32M","@time_based":""}"#;
        assert_variants(
            &convert("defaults_engine.json"),
            &job_variants("randread", defaults, &["read"]),
        );
        assert_variants(
            &convert("explicit_defaults.json"),
            &job_variants(
                "randread",
                &with(
                    defaults,
                    &[("@ioengine", "psync".into()), ("@iodepth", "1".into())],
                ),
                &["read"],
            ),
        );
    }

    /// Kills: fio's own `note:` and `fio:` lines refused before the JSON, and any other line
    /// before it skipped.
    #[test]
    fn adapter_storage_fio_json_leading_lines() {
        let results = convert("stdout_note.json");
        assert_value(
            &results,
            (
                "qd",
                r#"{"@bs":"4k","@direction":"read","@iodepth":"16","@ioengine":"psync","@runtime":"2","@rw":"randread","@size":"32M","@time_based":""}"#,
            ),
            (Throughput::SLUG_STR, "mean"),
            40_744.127_936,
        );
        assert!(convert("note_output_flag_3.36.json").has_metrics());
        let input = read("stdout_note.json");
        assert!(parse(&input.replacen("note:", "warning:", 1)).is_none());
        assert!(parse(&format!("starting\n{input}")).is_none());
    }

    /// Kills: the first or a middle report of `--status-interval` read instead of fio's final
    /// cumulative one, and a stream with text between its reports read.
    #[test]
    fn adapter_storage_fio_json_status_interval() {
        let results = convert("status_interval.json");
        assert_eq!(variants(&results), 4);
        assert_value(
            &results,
            (
                "randread",
                &with(RANDREAD, &[("@direction", "read".into())]),
            ),
            (Throughput::SLUG_STR, "mean"),
            41_795.602_199,
        );
        let input = read("status_interval.json");
        assert!(parse(&input.replacen("}\n{", "}\nnote: between\n{", 1)).is_none());
    }

    /// Kills: a percentile put on the wrong latency, a `percentile_list` entry of 100
    /// overwriting fio's exact maximum, and a percentile misnamed.
    #[test]
    fn adapter_storage_fio_json_percentiles() {
        let parameters = with(RANDREAD, &[("@direction", "read".into())]);
        let results = convert("percentile_list.json");
        assert_eq!(
            metric_names(
                &results,
                "randread",
                &parameters,
                CompletionLatency::SLUG_STR
            ),
            ["max", "mean", "median", "min", "p99_5", "p99_99", "std_dev"]
                .map(ToOwned::to_owned)
                .into()
        );

        let results = convert("percentile_list_100.json");
        assert_value(
            &results,
            ("pct", &parameters),
            (CompletionLatency::SLUG_STR, "max"),
            8_577_367.0,
        );
        assert_value(
            &results,
            ("pct", &parameters),
            (CompletionLatency::SLUG_STR, "p99"),
            125_440.0,
        );

        let latpct = r#"{"@bs":"4k","@direction":"read","@iodepth":"8","@ioengine":"io_uring","@runtime":"2","@rw":"randread","@size":"32M","@time_based":""}"#;
        let results = convert("lat_percentiles.json");
        assert_value(
            &results,
            ("latpct", latpct),
            (Latency::SLUG_STR, "median"),
            30336.0,
        );
        assert_eq!(
            value(
                &results,
                "latpct",
                latpct,
                CompletionLatency::SLUG_STR,
                "median"
            ),
            None
        );
    }

    /// Kills: submission latency dropped where it has samples, the sync direction dropped or
    /// read from the write direction, and `trim` or `mixed` dropped.
    #[test]
    fn adapter_storage_fio_json_directions() {
        let parameters = r#"{"@bs":"4k","@iodepth":"8","@ioengine":"io_uring","@runtime":"2","@rw":"randrw","@size":"32M","@time_based":""}"#;
        let results = convert("io_uring_slat.json");
        assert_variants(
            &results,
            &job_variants("async", parameters, &["read", "write"]),
        );
        for (direction, mean) in [("read", 4_401.743_779), ("write", 2_191.645_703)] {
            assert_value(
                &results,
                (
                    "async",
                    &with(parameters, &[("@direction", direction.into())]),
                ),
                (SubmissionLatency::SLUG_STR, "mean"),
                mean,
            );
        }

        let parameters = r#"{"@bs":"4k","@fsync":"8","@ioengine":"psync","@runtime":"2","@rw":"write","@size":"32M","@time_based":""}"#;
        let results = convert("fsync.json");
        assert_variants(
            &results,
            &job_variants("sync", parameters, &["write", "sync"]),
        );
        let sync = with(parameters, &[("@direction", "sync".into())]);
        assert_eq!(
            measure_names(&results, "sync", &sync),
            [Latency::SLUG_STR.to_owned()].into()
        );
        assert_value(
            &results,
            ("sync", &sync),
            (Latency::SLUG_STR, "mean"),
            6_623_109.132_89,
        );
        assert_value(
            &results,
            ("sync", &sync),
            (Latency::SLUG_STR, "p99_99"),
            19_005_440.0,
        );
        assert_value(
            &results,
            ("sync", &with(parameters, &[("@direction", "write".into())])),
            (Throughput::SLUG_STR, "mean"),
            1_203.398_301,
        );

        let trim = r#"{"@bs":"4k","@direct":"1","@runtime":"2","@rw":"randtrim","@time_based":""}"#;
        let results = convert("trim.json");
        assert_variants(&results, &job_variants("trim", trim, &["trim"]));
        assert_value(
            &results,
            ("trim", &with(trim, &[("@direction", "trim".into())])),
            (Throughput::SLUG_STR, "mean"),
            87080.0,
        );

        let mixed = r#"{"@bs":"4k","@ioengine":"psync","@runtime":"2","@rw":"randrw","@size":"32M","@time_based":""}"#;
        assert_variants(
            &convert("mixed.json"),
            &job_variants("mixed", mixed, &["mixed"]),
        );
        let results = convert("both.json");
        assert_variants(
            &results,
            &job_variants("both", mixed, &["read", "write", "mixed"]),
        );
        assert_value(
            &results,
            ("both", &with(mixed, &[("@direction", "mixed".into())])),
            (Throughput::SLUG_STR, "mean"),
            60614.0,
        );
    }

    /// Kills: a failed job with no count of its errors kept or left uncounted, a job with
    /// `continue_on_error` dropped or its errors unwritten, a good job dropped beside a failed
    /// one, the `window_*` metrics written where fio took no sample, and a run where no job did
    /// any I/O claimed.
    #[test]
    fn adapter_storage_fio_json_errors() {
        let results = convert("io_error_3.36.json");
        assert_eq!(results.skipped.benchmarks, 1);
        assert!(!results.has_metrics());

        let parameters =
            r#"{"@bs":"4k","@direct":"1","@runtime":"2","@rw":"randread","@size":"8M"}"#;
        let results = convert("continue_on_error_io_errors_3.36.json");
        assert_variants(&results, &job_variants("ioerr", parameters, &[]));
        assert_variant(
            &results,
            "ioerr",
            parameters,
            &[
                (ContextSwitches::SLUG_STR, vec![("value", 2.0)]),
                (IoErrors::SLUG_STR, vec![("value", 3_144_024.0)]),
                (MajorPageFaults::SLUG_STR, vec![("value", 0.0)]),
                (MinorPageFaults::SLUG_STR, vec![("value", 19.0)]),
            ],
        );
        let results = convert("continue_on_error_3.36.json");
        let parameters = r#"{"@bs":"4k","@rw":"read","@size":"64k"}"#;
        assert_value(
            &results,
            ("cerr", parameters),
            (IoErrors::SLUG_STR, "value"),
            0.0,
        );
        // fio took no sample window in so short a run.
        let direction = with(parameters, &[("@direction", "read".into())]);
        for measure in [Throughput::SLUG_STR, BytesThroughput::SLUG_STR] {
            assert_eq!(
                metric_names(&results, "cerr", &direction, measure),
                ["mean".to_owned()].into()
            );
        }

        let results = convert_edited("randread.json", |json| {
            let mut failed = json["jobs"][0].clone();
            failed["jobname"] = "failed".into();
            failed["error"] = 5.into();
            json["jobs"].as_array_mut().expect("jobs").insert(0, failed);
        });
        assert_eq!(results.skipped.benchmarks, 1);
        assert_variants(&results, &job_variants("randread", RANDREAD, &["read"]));

        assert!(parse(&read("error.json")).is_none());
    }

    /// Kills: a key fio added after 3.0 required (`N` in 3.18, `sync` in 3.4, `bw_bytes` in
    /// 3.2), a latency with no samples written where fio writes no `N`, the run's bandwidth not
    /// read from `bw` before `bw_bytes`, the `0.00` entries fio 3.0 pads its percentiles with
    /// overwriting fio's exact minimum, the pre-3.0 microsecond keys read, client/server output
    /// read, and output other than fio's claimed.
    #[test]
    fn adapter_storage_fio_json_versions() {
        let results = convert("randread_3.36.json");
        let parameters_3_36 =
            r#"{"@bs":"4k","@ioengine":"psync","@runtime":"2","@rw":"randread","@size":"32M"}"#;
        assert_variants(
            &results,
            &job_variants("randread", parameters_3_36, &["read"]),
        );

        let results = convert("randread_3.0.json");
        assert_variants(
            &results,
            &job_variants("randread", parameters_3_36, &["read"]),
        );
        let read_3_0 = with(parameters_3_36, &[("@direction", "read".into())]);
        assert_eq!(
            measure_names(&results, "randread", &read_3_0),
            [
                BytesThroughput::SLUG_STR,
                CompletionLatency::SLUG_STR,
                Latency::SLUG_STR,
                Throughput::SLUG_STR,
            ]
            .map(ToOwned::to_owned)
            .into()
        );
        for (measure, metric, expected) in [
            (BytesThroughput::SLUG_STR, "mean", 3488.0 * 1024.0),
            (CompletionLatency::SLUG_STR, "min", 26060.0),
            (CompletionLatency::SLUG_STR, "median", 108_032.0),
            (Throughput::SLUG_STR, "mean", 872.191_712),
        ] {
            assert_value(
                &results,
                ("randread", &read_3_0),
                (measure, metric),
                expected,
            );
        }

        let pre_3_0 = read("randread.json")
            .replace(r#""lat_ns""#, r#""lat""#)
            .replace(r#""clat_ns""#, r#""clat""#)
            .replace(r#""slat_ns""#, r#""slat""#);
        assert!(parse(&pre_3_0).is_none());
        let client_stats = read("randread.json").replace(r#""jobs""#, r#""client_stats""#);
        assert!(parse(&client_stats).is_none());
        let other =
            read("randread.json").replace(r#""fio version" : "fio-"#, r#""fio version" : "v"#);
        assert!(parse(&other).is_none());

        let results = convert("randrw_jsonplus.json");
        assert_variants(
            &results,
            &job_variants(
                "randrw",
                &RANDREAD.replace("randread", "randrw"),
                &["read", "write"],
            ),
        );
    }
}
