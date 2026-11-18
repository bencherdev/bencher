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

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::storage::fio::{Cpu, Direction, Job, Metrics, Outcome, Run, results},
    results::adapter_results::AdapterResults,
};

const KIB: f64 = 1024.0;

/// fio's `normal` text: the block fio prints for each job that issued I/O, under the header it
/// prints for every such job (`stat.c:1248`).
///
/// Each value is read at the precision fio prints it, and the run rate from fio's own formula
/// over the issued I/O and the runtime (`stat.c:1536`). Of the reports `--status-interval`
/// prints, the last, fio's final cumulative one, is read. The text cannot tell an option the
/// user set from a default, so it gives no workload parameters. A job that failed before any
/// I/O has no block, only fio's error line, which counts it only when the input holds that line:
/// fio writes a failure in its parent process to stderr (`backend.c:2730`), and without it a
/// repeated name can shift its `@job`.
pub struct AdapterStorageFioText;

impl Adaptable for AdapterStorageFioText {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut jobs = Vec::new();
        let mut failed = Vec::new();
        let mut block: Option<Block<'_>> = None;
        let mut reported = false;
        for line in input.lines() {
            if let Some(header) = Header::parse(line) {
                jobs.extend(block.take().map(Block::job));
                if reported {
                    jobs.clear();
                    reported = false;
                }
                block = Some(Block::new(header));
            } else if line.trim().is_empty() || line.starts_with("Run status group ") {
                jobs.extend(block.take().map(Block::job));
                reported |= line.starts_with("Run status group ");
            } else if let Some(pid) = error_pid(line) {
                failed.push(pid);
            } else if let Some(block) = &mut block {
                block.read(line.trim());
            }
        }
        jobs.extend(block.map(Block::job));
        if jobs.is_empty() {
            return None;
        }
        let (pids, jobs): (Vec<u32>, Vec<Job>) = jobs.into_iter().unzip();
        let unwritten = failed.iter().filter(|pid| !pids.contains(pid)).count();
        results(&jobs, unwritten, Adapter::StorageFioText)
    }
}

/// The pid of fio's error line for a job that ended in an error (`backend.c:2366`, `2730`):
/// `fio: pid=<n>, err=<n>/<text>`.
fn error_pid(line: &str) -> Option<u32> {
    let (pid, _) = line.strip_prefix("fio: pid=")?.split_once(", err=")?;
    pid.parse().ok()
}

/// A job's header: `<name>: (groupid=<n>, jobs=<n>): err=<n>`, then the error's text when it is
/// not 0, then `: pid=<n>: <time>`.
struct Header<'a> {
    name: &'a str,
    error: i64,
    pid: u32,
}

impl<'a> Header<'a> {
    fn parse(line: &'a str) -> Option<Self> {
        let (name, rest) = line.split_once(": (groupid=")?;
        let (group, rest) = rest.split_once(", jobs=")?;
        let (jobs, rest) = rest.split_once("): err=")?;
        let rest = rest.trim_start();
        let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '-'))?;
        let (error, rest) = rest.split_at(end);
        let numbers = [group, jobs]
            .iter()
            .all(|number| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()));
        if name.is_empty() || !numbers || !(rest.starts_with(": pid=") || rest.starts_with(" (")) {
            return None;
        }
        let (_, pid) = rest.rsplit_once(": pid=")?;
        let (pid, _) = pid.split_once(':')?;
        Some(Self {
            name,
            error: error.parse().ok()?,
            pid: pid.parse().ok()?,
        })
    }
}

/// The lines of one job's block.
struct Block<'a> {
    header: Header<'a>,
    metrics: Metrics,
    directions: Vec<DirectionBlock>,
    percentiles: Option<(MeasureNameId, f64)>,
    issued: Option<[f64; 3]>,
    cpu: Option<Cpu>,
    errors: Option<f64>,
}

impl<'a> Block<'a> {
    fn new(header: Header<'a>) -> Self {
        Self {
            header,
            metrics: Metrics::default(),
            directions: Vec::new(),
            percentiles: None,
            issued: None,
            cpu: None,
            errors: None,
        }
    }

    fn read(&mut self, line: &str) {
        let percentiles = self.percentiles.take();
        if let Some(entries) = line.strip_prefix('|') {
            if let (Some((measure, factor)), Some(direction)) =
                (percentiles, self.directions.last_mut())
            {
                for entry in entries.split(',').map(str::trim).filter(|e| !e.is_empty()) {
                    direction.percentile(&measure, factor, entry);
                }
                self.percentiles = Some((measure, factor));
            }
        } else if let Some((direction, rest)) = line.split_once(": IOPS=") {
            if let Some(direction) = io_direction(direction) {
                self.directions.push(DirectionBlock::new(direction, rest));
            }
        } else if line == "fsync/fdatasync/sync_file_range/syncfs:" {
            self.directions.push(DirectionBlock {
                direction: Direction::Sync,
                metrics: Metrics::default(),
                runtime: None,
                kb_base: None,
            });
        } else if let Some((kind, unit)) = line
            .strip_suffix("):")
            .and_then(|line| line.split_once(" percentiles ("))
        {
            self.percentiles = latency_measure(kind).zip(nanoseconds(unit));
        } else if let Some(rest) = line.strip_prefix("bw (") {
            if let Some(direction) = self.directions.last_mut() {
                direction.bandwidth(rest);
            }
        } else if let Some(rest) = field_line(line, "iops") {
            if let Some(direction) = self.directions.last_mut() {
                direction.iops(rest);
            }
        } else if let Some(rest) = field_line(line, "cpu") {
            self.read_cpu(rest);
        } else if let Some(rest) = line
            .strip_prefix("issued rwts: total=")
            .or_else(|| line.strip_prefix("issued rwt: total="))
        {
            self.issued = issued(rest);
        } else if let Some(rest) = field_line(line, "errors") {
            self.errors = fields(rest)
                .find(|(key, _)| *key == "total")
                .and_then(|(_, value)| value.parse().ok());
        } else if let Some((kind, rest)) = line.split_once(" (")
            && let (Some(measure), Some(direction)) =
                (latency_measure(kind), self.directions.last_mut())
        {
            direction.latency(&measure, rest);
        }
    }

    /// `usr=<n>%, sys=<n>%, ctx=<n>, majf=<n>, minf=<n>`
    fn read_cpu(&mut self, rest: &str) {
        let percent = |value: &str| value.strip_suffix('%')?.parse::<f64>().ok();
        let (mut user, mut system) = (None, None);
        for (key, value) in fields(rest) {
            match key {
                "usr" => user = percent(value),
                "sys" => system = percent(value),
                "ctx" => self.count(&ContextSwitches::name_id(), value),
                "majf" => self.count(&MajorPageFaults::name_id(), value),
                "minf" => self.count(&MinorPageFaults::name_id(), value),
                _ => {},
            }
        }
        self.cpu = user.zip(system).map(|(user, system)| Cpu { user, system });
    }

    fn count(&mut self, measure: &MeasureNameId, value: &str) {
        if let Ok(value) = value.parse() {
            self.metrics.insert(measure, "value", value);
        } else {
            self.metrics.skip_metric();
        }
    }

    /// The job, by its pid, skipped and counted when it failed with no count of its errors.
    fn job(self) -> (u32, Job) {
        let Self {
            header,
            mut metrics,
            directions,
            issued,
            cpu,
            errors,
            ..
        } = self;
        let outcome = if header.error != 0 && errors.is_none() {
            Outcome::Failed
        } else {
            if let Some(errors) = errors {
                metrics.insert(&IoErrors::name_id(), "value", errors);
            }
            Outcome::Ran(Run {
                options: serde_json::Map::new(),
                metrics,
                directions: directions
                    .into_iter()
                    .map(|direction| direction.finish(issued))
                    .collect(),
                cpu,
            })
        };
        (
            header.pid,
            Job {
                name: header.name.to_owned(),
                outcome,
            },
        )
    }
}

/// One direction's lines, with the runtime and the KiB base of its rate line.
struct DirectionBlock {
    direction: Direction,
    metrics: Metrics,
    runtime: Option<f64>,
    kb_base: Option<f64>,
}

impl DirectionBlock {
    /// A direction from its rate line: `IOPS=<n>, BW=<rate> (<rate>)(<size>/<n>msec)`.
    fn new(direction: Direction, rest: &str) -> Self {
        let mut block = Self {
            direction,
            metrics: Metrics::default(),
            runtime: None,
            kb_base: None,
        };
        let rate = rest
            .split_once(", BW=")
            .and_then(|(_, rest)| rest.split_once(" ("))
            .and_then(|(rate, _)| bytes_per_second(rate));
        if let Some((rate, kb_base)) = rate {
            block
                .metrics
                .insert(&BytesThroughput::name_id(), "mean", rate);
            block.kb_base = kb_base;
        } else {
            block.metrics.skip_metric();
        }
        block.runtime = rest
            .split_once("msec)")
            .and_then(|(rest, _)| rest.rsplit_once('/'))
            .and_then(|(_, runtime)| runtime.parse().ok());
        block
    }

    /// `(<unit>): min=<n>, max=<n>, avg=<n>, stdev=<n>`
    fn latency(&mut self, measure: &MeasureNameId, rest: &str) {
        let Some((unit, rest)) = rest.split_once("): min=") else {
            return;
        };
        let summary = nanoseconds(unit).and_then(|factor| {
            let (min, rest) = rest.split_once(", max=")?;
            let (max, rest) = rest.split_once(", avg=")?;
            let (mean, std_dev) = rest.split_once(", stdev=")?;
            Some([
                si(min)? * factor,
                si(max)? * factor,
                mean.trim().parse::<f64>().ok()? * factor,
                std_dev.trim().parse::<f64>().ok()? * factor,
            ])
        });
        if let Some(summary) = summary {
            self.metrics.insert_summary(measure, summary);
        } else {
            self.metrics.skip_metric();
        }
    }

    /// `<percent>th=[<n>]`
    fn percentile(&mut self, measure: &MeasureNameId, factor: f64, entry: &str) {
        let percentile = entry.split_once("th=[").and_then(|(percent, value)| {
            let value = value.strip_suffix(']')?.trim().parse::<f64>().ok()?;
            Some((percent, value * factor))
        });
        if let Some((percent, value)) = percentile {
            self.metrics.insert_percentile(measure, percent, value);
        } else {
            self.metrics.skip_metric();
        }
    }

    /// `<unit>/s): min=<n>, max=<n>, per=<n>%, avg=<n>, stdev=<n>, samples=<n>`, where fio's
    /// samples are in KiB/s (`stat.c:3622`) whatever `kb_base` labels them, and are divided by
    /// `kb_base` once their mean passes `kb_base` squared.
    fn bandwidth(&mut self, rest: &str) {
        let Some((unit, rest)) = rest.split_once("/s): ") else {
            return;
        };
        let factor = match unit.trim() {
            "KiB" | "kB" => Some(KIB),
            "MiB" => self.kb_base.map(|kb_base| KIB * kb_base),
            _ => None,
        };
        self.windows(&BytesThroughput::name_id(), factor, rest);
    }

    /// `min=<n>, max=<n>, avg=<n>, stdev=<n>, samples=<n>`
    fn iops(&mut self, rest: &str) {
        self.windows(&Throughput::name_id(), Some(1.0), rest);
    }

    fn windows(&mut self, measure: &MeasureNameId, factor: Option<f64>, rest: &str) {
        for (key, metric) in [
            ("min", "window_min"),
            ("max", "window_max"),
            ("avg", "window_mean"),
            ("stdev", "window_std_dev"),
        ] {
            let value = fields(rest)
                .find(|(field, _)| *field == key)
                .and_then(|(_, value)| value.parse::<f64>().ok())
                .zip(factor);
            if let Some((value, factor)) = value {
                self.metrics.insert(measure, metric, value * factor);
            } else {
                self.metrics.skip_metric();
            }
        }
    }

    /// The direction, with its run rate from the I/O it issued over its runtime, the `mixed`
    /// direction's from every direction's I/O.
    fn finish(mut self, issued: Option<[f64; 3]>) -> (Direction, Metrics) {
        let total = issued.and_then(|[read, write, trim]| match self.direction {
            Direction::Read => Some(read),
            Direction::Write => Some(write),
            Direction::Trim => Some(trim),
            Direction::Mixed => Some(read + write + trim),
            Direction::Sync => None,
        });
        if let Some(runtime) = self.runtime {
            if let Some(total) = total.filter(|_| runtime > 0.0) {
                self.metrics
                    .insert(&Throughput::name_id(), "mean", 1000.0 * total / runtime);
            } else {
                self.metrics.skip_metric();
            }
        }
        (self.direction, self.metrics)
    }
}

/// The measure of a latency fio prints as `kind`: `sync` is the sync direction's latency.
fn latency_measure(kind: &str) -> Option<MeasureNameId> {
    match kind {
        "lat" | "sync" => Some(Latency::name_id()),
        "clat" => Some(CompletionLatency::name_id()),
        "slat" => Some(SubmissionLatency::name_id()),
        _ => None,
    }
}

fn io_direction(name: &str) -> Option<Direction> {
    match name {
        "read" => Some(Direction::Read),
        "write" => Some(Direction::Write),
        "trim" => Some(Direction::Trim),
        "mixed" => Some(Direction::Mixed),
        _ => None,
    }
}

/// The factor to nanoseconds of a unit fio prints a latency in.
fn nanoseconds(unit: &str) -> Option<f64> {
    match unit {
        "nsec" => Some(1.0),
        "usec" => Some(1e3),
        "msec" => Some(1e6),
        _ => None,
    }
}

/// The text after `<name>` and the spaces and `: ` that align it, as in `cpu          : `.
fn field_line<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    line.strip_prefix(name)?.trim_start().strip_prefix(": ")
}

/// The `key=value` fields of a line, separated by `, `.
fn fields(rest: &str) -> impl Iterator<Item = (&str, &str)> {
    rest.split(", ")
        .filter_map(|field| field.split_once('='))
        .map(|(key, value)| (key.trim(), value.trim()))
}

/// The read, write, and trim totals of `<read>,<write>,<trim>,<sync> short=...`, or of fio
/// 3.0's `<read>,<write>,<trim>, short=...`.
fn issued(rest: &str) -> Option<[f64; 3]> {
    let (totals, _) = rest.split_once(' ').unwrap_or((rest, ""));
    let mut totals = totals.split(',').map(|total| total.parse::<f64>().ok());
    Some([totals.next()??, totals.next()??, totals.next()??])
}

/// A number fio's `num2str` prints with no unit, with its SI prefix, as `19307k`.
fn si(text: &str) -> Option<f64> {
    let (value, rest, _) = prefixed(text.trim())?;
    rest.is_empty().then_some(value)
}

/// A rate fio's `num2str` prints, in bytes per second, with the KiB base its prefix implies: in
/// `B/s`, in `bit/s`, or with no unit, as fio prints the `mixed` direction under
/// `unified_rw_reporting=both`.
fn bytes_per_second(text: &str) -> Option<(f64, Option<f64>)> {
    let (value, unit, kb_base) = prefixed(text)?;
    match unit {
        "B/s" | "" => Some((value, kb_base)),
        "bit/s" => Some((value / 8.0, kb_base)),
        _ => None,
    }
}

/// A number with its unit prefix applied, the rest of the text, and the KiB base the prefix
/// implies: 1024 for an IEC prefix, 1000 for an SI one.
fn prefixed(text: &str) -> Option<(f64, &str, Option<f64>)> {
    const IEC: [&str; 6] = ["Ki", "Mi", "Gi", "Ti", "Pi", "Ei"];
    const SI: [&str; 6] = ["k", "M", "G", "T", "P", "E"];
    let end = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    let (number, rest) = text.split_at(end);
    let number = number.parse::<f64>().ok()?;
    for (prefixes, base) in [(IEC, KIB), (SI, 1000.0)] {
        let mut factor = 1.0;
        for prefix in prefixes {
            factor *= base;
            if let Some(rest) = rest.strip_prefix(prefix) {
                return Some((number * factor, rest, Some(base)));
            }
        }
    }
    Some((number, rest, None))
}

#[cfg(test)]
mod test_storage_fio_text {
    use std::collections::BTreeSet;

    use bencher_json::{
        BmfVersion, ParameterSet,
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
            assert_ignores_average, assert_value, assert_variant, convert_v1, value, variants,
        },
        results::{adapter_metrics::AdapterMetrics, adapter_results::AdapterResults},
    };

    const FIO: &str = "storage/fio";
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;

    fn convert(fixture: &str) -> AdapterResults {
        convert_v1(Adapter::StorageFioText, &format!("{FIO}/{fixture}"))
    }

    fn parse(adapter: Adapter, input: &str) -> Option<AdapterResults> {
        adapter.convert(input, Settings::new(None, BmfVersion::V1))
    }

    fn read(fixture: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/{FIO}/{fixture}"))
            .expect("Failed to read a fixture")
    }

    fn direction(direction: &str) -> String {
        format!(r#"{{"@direction":"{direction}"}}"#)
    }

    fn job(job: usize, direction: Option<&str>) -> String {
        direction.map_or_else(
            || format!(r#"{{"@job":{job}}}"#),
            |direction| format!(r#"{{"@direction":"{direction}","@job":{job}}}"#),
        )
    }

    /// The variants of one job without workload parameters: its own, one per direction that
    /// did I/O, and one per CPU mode.
    fn job_variants(name: &str, extra: &str, directions: &[&str]) -> BTreeSet<(String, String)> {
        let mut parameters = vec![
            format!("{{{extra}}}"),
            format!(r#"{{"@cpu":"user"{}}}"#, comma(extra)),
            format!(r#"{{"@cpu":"system"{}}}"#, comma(extra)),
        ];
        for direction in directions {
            parameters.push(format!(r#"{{"@direction":"{direction}"{}}}"#, comma(extra)));
        }
        parameters
            .into_iter()
            .map(|parameters| {
                let parameters: ParameterSet =
                    parameters.parse().expect("Failed to parse parameters");
                (name.to_owned(), parameters.canonical())
            })
            .collect()
    }

    fn comma(extra: &str) -> String {
        if extra.is_empty() {
            String::new()
        } else {
            format!(",{extra}")
        }
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

    /// The JSON leaf's reading of the same run, in `randrw_json_normal.txt`, agrees with the
    /// text leaf's on every metric the text leaf reads, to within the text's printed precision,
    /// and the text leaf reads every metric the JSON leaf reads.
    ///
    /// Kills: a statistic dropped, misnamed, scaled, or put on the wrong measure or variant, and
    /// the run rate not taken from fio's own formula over the issued I/O and the runtime.
    #[test]
    fn adapter_storage_fio_text_matches_json() {
        let input = read("randrw_json_normal.txt");
        let lines: Vec<&str> = input.lines().collect();
        let start = lines.iter().position(|line| *line == "{").expect("JSON");
        let end = lines.iter().position(|line| *line == "}").expect("JSON");
        let json = parse(Adapter::StorageFioJson, &lines[start..=end].join("\n")).expect("JSON");
        let text = convert("randrw_json_normal.txt");
        let workload = r#""@bs":"4k","@ioengine":"psync","@runtime":"2","@rw":"randrw","@size":"32M","@time_based":"""#;
        assert_eq!(variants(&text), 5);
        for (name, variants) in &text.inner {
            for (parameters, metrics) in variants {
                let mut parameters: serde_json::Map<String, serde_json::Value> =
                    serde_json::from_str(&parameters.canonical()).expect("parameters");
                let workload: serde_json::Map<String, serde_json::Value> =
                    serde_json::from_str(&format!("{{{workload}}}")).expect("workload");
                parameters.extend(workload);
                let parameters: ParameterSet = serde_json::Value::Object(parameters)
                    .to_string()
                    .parse()
                    .expect("parameters");
                let expected = &json.inner[name][&parameters];
                let names = |metrics: &AdapterMetrics| {
                    metrics
                        .inner
                        .iter()
                        .flat_map(|(measure, metric)| {
                            metric
                                .inner
                                .keys()
                                .map(move |metric| (measure.to_string(), metric.to_string()))
                        })
                        .collect::<BTreeSet<_>>()
                };
                assert_eq!(names(metrics), names(expected), "{name} {parameters}");
                for (measure, metric) in &metrics.inner {
                    for (name, value) in &metric.inner {
                        let actual = value.into_inner();
                        let expected = expected.inner[measure].inner[name].into_inner();
                        let tolerance = tolerance(&measure.to_string(), name.as_ref(), expected);
                        assert!(
                            (actual - expected).abs() <= tolerance,
                            "{parameters} {measure} {name}: {actual} != {expected}"
                        );
                    }
                }
            }
        }
    }

    /// How far fio's text may print a value from its JSON: a latency to its display unit (here
    /// whole microseconds), a rate's mean to its 3 significant figures, and the rest to the
    /// two decimals fio prints.
    fn tolerance(measure: &str, metric: &str, expected: f64) -> f64 {
        if measure == BytesThroughput::SLUG_STR && metric == "mean" {
            return expected * 0.005;
        }
        if measure == BytesThroughput::SLUG_STR {
            return 0.005 * KIB;
        }
        if [
            Latency::SLUG_STR,
            CompletionLatency::SLUG_STR,
            SubmissionLatency::SLUG_STR,
        ]
        .contains(&measure)
        {
            return 1000.0;
        }
        if measure == Throughput::SLUG_STR && metric == "mean" {
            return expected * 1e-9;
        }
        0.005
    }

    /// Kills: a statistic dropped, misnamed, or scaled from fio's display unit, and the
    /// window statistics or the run rate read from the wrong line.
    #[test]
    fn adapter_storage_fio_text_statistics() {
        let results = convert("randrw_normal.txt");
        assert_eq!(results.resolved_adapter, Some(Adapter::StorageFioText));
        assert_eq!(
            variant_names(&results),
            job_variants("randrw", "", &["read", "write"])
        );
        assert_variant(
            &results,
            "randrw",
            &direction("read"),
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("min", 13000.0),
                        ("max", 472_000.0),
                        ("mean", 23240.0),
                        ("std_dev", 4810.0),
                    ],
                ),
                (
                    CompletionLatency::SLUG_STR,
                    vec![
                        ("min", 13000.0),
                        ("max", 472_000.0),
                        ("mean", 23220.0),
                        ("std_dev", 4810.0),
                        ("p1", 15000.0),
                        ("p5", 16000.0),
                        ("p10", 18000.0),
                        ("p20", 21000.0),
                        ("p30", 22000.0),
                        ("p40", 22000.0),
                        ("median", 24000.0),
                        ("p60", 25000.0),
                        ("p70", 26000.0),
                        ("p80", 27000.0),
                        ("p90", 28000.0),
                        ("p95", 29000.0),
                        ("p99", 35000.0),
                        ("p99_5", 40000.0),
                        ("p99_9", 58000.0),
                        ("p99_95", 70000.0),
                        ("p99_99", 113_000.0),
                    ],
                ),
                (
                    Throughput::SLUG_STR,
                    vec![
                        ("mean", 1000.0 * 72844.0 / 2001.0),
                        ("window_min", 34782.0),
                        ("window_max", 37522.0),
                        ("window_mean", 36403.75),
                        ("window_std_dev", 1270.6),
                    ],
                ),
                (
                    BytesThroughput::SLUG_STR,
                    vec![
                        ("mean", 142.0 * MIB),
                        ("window_min", 139_128.0 * KIB),
                        ("window_max", 150_088.0 * KIB),
                        ("window_mean", 145_615.75 * KIB),
                        ("window_std_dev", 5082.07 * KIB),
                    ],
                ),
            ],
        );
        assert_variant(
            &results,
            "randrw",
            "{}",
            &[
                (ContextSwitches::SLUG_STR, vec![("value", 73587.0)]),
                (MajorPageFaults::SLUG_STR, vec![("value", 0.0)]),
                (MinorPageFaults::SLUG_STR, vec![("value", 10.0)]),
            ],
        );
        for (cpu, expected) in [("user", 2.5), ("system", 25.25)] {
            assert_variant(
                &results,
                "randrw",
                &format!(r#"{{"@cpu":"{cpu}"}}"#),
                &[(CpuUtilization::SLUG_STR, vec![("value", expected)])],
            );
        }
        assert_value(
            &results,
            ("randrw", &direction("write")),
            (CompletionLatency::SLUG_STR, "mean"),
            1009.1,
        );
        assert_value(
            &results,
            ("randrw", &direction("write")),
            (Throughput::SLUG_STR, "mean"),
            1000.0 * 72341.0 / 2001.0,
        );
        assert_ignores_average(Adapter::StorageFioText, &format!("{FIO}/randrw_normal.txt"));
    }

    /// Kills: a latency in milliseconds or with fio's `k` suffix misread, submission latency
    /// dropped, the samples' bandwidth in MiB/s or under `kb_base=1000` (in KiB/s, or in units of
    /// 1000 KiB/s once scaled) misread, and the run's bandwidth misread in GiB/s, SI units, bits,
    /// or the unitless `mixed` line.
    #[test]
    fn adapter_storage_fio_text_units() {
        let results = convert("msec_normal.txt");
        let read = direction("read");
        for (measure, metric, expected) in [
            (CompletionLatency::SLUG_STR, "min", 151e6),
            (CompletionLatency::SLUG_STR, "mean", 228.09e6),
            (CompletionLatency::SLUG_STR, "p1", 153e6),
            (BytesThroughput::SLUG_STR, "mean", 17.5 * KIB),
            (BytesThroughput::SLUG_STR, "window_min", 16.0 * KIB),
            (Throughput::SLUG_STR, "mean", 1000.0 * 14.0 / 3194.0),
        ] {
            assert_value(&results, ("slow", &read), (measure, metric), expected);
        }

        let results = convert("io_uring_slat_normal.txt");
        for (measure, metric, expected) in [
            (CompletionLatency::SLUG_STR, "max", 19_307_000.0),
            (CompletionLatency::SLUG_STR, "p99_99", 17_957_000.0),
            (SubmissionLatency::SLUG_STR, "min", 612.0),
            (SubmissionLatency::SLUG_STR, "mean", 4738.89),
        ] {
            assert_value(&results, ("async", &read), (measure, metric), expected);
        }

        let results = convert("lat_percentiles_normal.txt");
        assert_value(
            &results,
            ("latpct", &read),
            (CompletionLatency::SLUG_STR, "max"),
            1_663_200.0,
        );
        assert_value(
            &results,
            ("latpct", &read),
            (Latency::SLUG_STR, "median"),
            31000.0,
        );
        assert_eq!(
            value(
                &results,
                "latpct",
                &read,
                CompletionLatency::SLUG_STR,
                "median"
            ),
            None
        );

        for (fixture, name, mean, window_min) in [
            (
                "null_normal.txt",
                "null",
                7881.0 * MIB * KIB,
                7_996_612.0 * MIB,
            ),
            ("kb_base_1000_normal.txt", "kb", 182e6, 168_168.0 * KIB),
            (
                "null_kb_base_1000_normal.txt",
                "null",
                7903e9,
                7_638_661.0 * KIB * 1000.0,
            ),
            (
                "unit_base_bits_normal.txt",
                "bits",
                1351.0 * MIB / 8.0,
                157_176.0 * KIB,
            ),
        ] {
            let results = convert(fixture);
            assert_value(
                &results,
                (name, &read),
                (BytesThroughput::SLUG_STR, "mean"),
                mean,
            );
            assert_value(
                &results,
                (name, &read),
                (BytesThroughput::SLUG_STR, "window_min"),
                window_min,
            );
        }

        let results = convert("both_normal.txt");
        assert_eq!(
            variant_names(&results),
            job_variants("both", "", &["read", "write", "mixed"])
        );
        let mixed = direction("mixed");
        assert_value(
            &results,
            ("both", &mixed),
            (BytesThroughput::SLUG_STR, "mean"),
            224.0 * MIB,
        );
        assert_value(
            &results,
            ("both", &mixed),
            (Throughput::SLUG_STR, "mean"),
            1000.0 * (57358.0 + 57132.0) / 2001.0,
        );
    }

    /// Kills: `@job` dropped from jobs that repeat a name, numbered out of fio's order, or put on
    /// a job whose name is unique, and a job's `Description` line ending its block.
    #[test]
    fn adapter_storage_fio_text_jobs() {
        let results = convert("numjobs4_normal.txt");
        let mut expected = BTreeSet::new();
        for (index, (total, runtime)) in [
            (737_281.0, 2001.0),
            (713_460.0, 2000.0),
            (712_705.0, 2005.0),
            (683_668.0, 2002.0),
        ]
        .into_iter()
        .enumerate()
        {
            expected.extend(job_variants("w", &format!(r#""@job":{index}"#), &["write"]));
            assert_value(
                &results,
                ("w", &job(index, Some("write"))),
                (Throughput::SLUG_STR, "mean"),
                1000.0 * total / runtime,
            );
        }
        assert_eq!(variant_names(&results), expected);
        assert!(
            value(
                &results,
                "w",
                &job(0, None),
                ContextSwitches::SLUG_STR,
                "value"
            )
            .is_some()
        );

        let results = convert("jobfile_two_jobs_normal.txt");
        let mut expected = job_variants("seq-read", "", &["read"]);
        expected.extend(job_variants("rand-write", "", &["write"]));
        assert_eq!(variant_names(&results), expected);
        assert_value(
            &results,
            ("rand-write", &direction("write")),
            (Throughput::SLUG_STR, "mean"),
            1000.0 * 483_971.0 / 2001.0,
        );
    }

    /// fio prints no block for a job that failed before any I/O, only its error line.
    ///
    /// Kills: such an error line not counted, or one count for any number of them, a job that
    /// repeats a name kept beside it, and a job whose name is unique dropped beside it.
    #[test]
    fn adapter_storage_fio_text_jobs_without_block() {
        let results = convert("numjobs4_open_fail_normal.txt");
        assert_eq!(results.skipped.benchmarks, 4);
        assert!(!results.has_metrics());

        let results = convert("unique_open_fail_normal.txt");
        assert_eq!(results.skipped.benchmarks, 4);
        assert_eq!(variant_names(&results), job_variants("good", "", &["read"]));
        assert_value(
            &results,
            ("good", &direction("read")),
            (Throughput::SLUG_STR, "mean"),
            1000.0 * 16066.0 / 1004.0,
        );

        let input = read("unique_open_fail_normal.txt").replacen(
            "fio: pid=1612045,",
            "fio: pid=1612047, err=13/file:filesetup.c:800, func=open(/tmp/bench/w.3.0), error=Permission denied\nfio: pid=1612045,",
            1,
        );
        let results =
            parse(Adapter::StorageFioText, &input).expect("Failed to parse 2 error lines");
        assert_eq!(results.skipped.benchmarks, 5);
        assert_eq!(variant_names(&results), job_variants("good", "", &["read"]));
    }

    /// Kills: fio 3.0's issued I/O line, `issued rwt:` with 3 totals, left unread.
    #[test]
    fn adapter_storage_fio_text_version_3_0() {
        let results = convert("randrw_3.0_normal.txt");
        assert_eq!(
            variant_names(&results),
            job_variants("randrw", "", &["read", "write"])
        );
        for (name, total) in [("read", 966_921.0), ("write", 965_716.0)] {
            assert_value(
                &results,
                ("randrw", &direction(name)),
                (Throughput::SLUG_STR, "mean"),
                1000.0 * total / 2001.0,
            );
        }
    }

    /// Kills: an earlier report of `--status-interval` read, or its jobs read as repeats.
    #[test]
    fn adapter_storage_fio_text_status_interval() {
        let results = convert("status_interval_normal.txt");
        assert_eq!(
            variant_names(&results),
            job_variants("randread", "", &["read"])
        );
        assert_value(
            &results,
            ("randread", &direction("read")),
            (Throughput::SLUG_STR, "mean"),
            1000.0 * 84725.0 / 2001.0,
        );
    }

    /// Kills: the sync block dropped or read into the write direction, a percentile of 100
    /// overwriting fio's exact maximum, a percentile with no decimals misnamed, `trim` or `mixed`
    /// dropped, and the `trim` or `mixed` run rate read from another direction's I/O.
    #[test]
    fn adapter_storage_fio_text_directions() {
        let results = convert("fsync_normal.txt");
        assert_eq!(
            variant_names(&results),
            job_variants("sync", "", &["write", "sync"])
        );
        assert_variant(
            &results,
            "sync",
            &direction("sync"),
            &[(
                Latency::SLUG_STR,
                vec![
                    ("min", 2_283_000.0),
                    ("max", 15_275_000.0),
                    ("mean", 3_187_260.0),
                    ("std_dev", 762_140.0),
                    ("p1", 2_343_000.0),
                    ("p5", 2_409_000.0),
                    ("p10", 2_474_000.0),
                    ("p20", 2_540_000.0),
                    ("p30", 2_638_000.0),
                    ("p40", 3_195_000.0),
                    ("median", 3_326_000.0),
                    ("p60", 3_392_000.0),
                    ("p70", 3_458_000.0),
                    ("p80", 3_523_000.0),
                    ("p90", 3_752_000.0),
                    ("p95", 4_146_000.0),
                    ("p99", 4_490_000.0),
                    ("p99_5", 6_718_000.0),
                    ("p99_9", 15_270_000.0),
                    ("p99_95", 15_270_000.0),
                    ("p99_99", 15_270_000.0),
                ],
            )],
        );
        assert_value(
            &results,
            ("sync", &direction("write")),
            (Latency::SLUG_STR, "max"),
            204_955.0,
        );

        let results = convert("percentile_list_100_normal.txt");
        let read = direction("read");
        for (metric, expected) in [("max", 974_000.0), ("median", 24000.0), ("p99", 45000.0)] {
            assert_value(
                &results,
                ("pct", &read),
                (CompletionLatency::SLUG_STR, metric),
                expected,
            );
        }

        let results = convert("trim_normal.txt");
        assert_eq!(variant_names(&results), job_variants("trim", "", &["trim"]));
        assert_value(
            &results,
            ("trim", &direction("trim")),
            (Throughput::SLUG_STR, "mean"),
            1000.0 * 235_442.0 / 2000.0,
        );
        let results = convert("mixed_normal.txt");
        assert_eq!(
            variant_names(&results),
            job_variants("mixed", "", &["mixed"])
        );
        assert_value(
            &results,
            ("mixed", &direction("mixed")),
            (Throughput::SLUG_STR, "mean"),
            1000.0 * 112_357.0 / 2001.0,
        );
    }

    /// Kills: a failed job with no count of its errors kept, left uncounted, or counted again for
    /// its error line, and a job with `continue_on_error` dropped or its errors unwritten.
    #[test]
    fn adapter_storage_fio_text_errors() {
        let results = convert("io_error_normal.txt");
        assert_eq!(results.skipped.benchmarks, 1);
        assert!(!results.has_metrics());

        let results = convert("continue_on_error_io_errors_normal.txt");
        assert_eq!(variant_names(&results), job_variants("ioerr", "", &[]));
        assert_variant(
            &results,
            "ioerr",
            "{}",
            &[
                (ContextSwitches::SLUG_STR, vec![("value", 2.0)]),
                (IoErrors::SLUG_STR, vec![("value", 2_897_790.0)]),
                (MajorPageFaults::SLUG_STR, vec![("value", 0.0)]),
                (MinorPageFaults::SLUG_STR, vec![("value", 17.0)]),
            ],
        );
        assert_value(
            &results,
            ("ioerr", r#"{"@cpu":"system"}"#),
            (CpuUtilization::SLUG_STR, "value"),
            53.7,
        );
    }

    /// Kills: a leaf that reads fio's lines without the job header it is sure of, and one that
    /// claims fio's terse or JSON output, or fio's error line before its JSON.
    #[test]
    fn adapter_storage_fio_text_gate() {
        let input = read("randrw_normal.txt");
        let headless = input
            .lines()
            .filter(|line| !line.contains("(groupid="))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(parse(Adapter::StorageFioText, &headless).is_none());
        for fixture in ["randrw_terse.txt", "randread.json", "open_fail.json"] {
            assert!(
                parse(Adapter::StorageFioText, &read(fixture)).is_none(),
                "{fixture}"
            );
        }
    }

    /// The node reads JSON with the JSON leaf and text with the text leaf.
    ///
    /// Kills: a node that drops either leaf.
    #[test]
    fn adapter_storage_fio_node() {
        for (fixture, leaf) in [
            ("randread.json", Adapter::StorageFioJson),
            ("randrw_normal.txt", Adapter::StorageFioText),
        ] {
            let results = parse(Adapter::StorageFio, &read(fixture)).expect("fio");
            assert_eq!(results.resolved_adapter, Some(leaf), "{fixture}");
        }
    }
}
