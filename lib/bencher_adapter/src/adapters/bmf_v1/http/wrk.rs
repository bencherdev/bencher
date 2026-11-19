use bencher_json::{
    BenchmarkName, MeasureNameId,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{BytesThroughput, Latency, Throughput},
            http::{
                ConnectErrors, ErrorResponses, FailedRequests, ReadErrors, Requests, Timeouts,
                UncorrectedLatency, WriteErrors,
            },
        },
        report::Adapter,
    },
};
use serde_json::{Map, Value};

use crate::{
    Adaptable, Settings,
    adapters::{
        bmf_v1::{
            finite::Finite,
            leaf::{LeafResults, Variant, parameter_set},
            percentile::percentile_name,
        },
        util::Units,
    },
    results::adapter_results::AdapterResults,
};

/// wrk and wrk2 at version 1: each run's report, named by the URL its header prints, with the
/// threads, connections, and duration it prints as parameters.
///
/// A run's header is its `Running` line and its threads line, then its `Thread Stats` header
/// after any lines the threads print while they run, such as wrk2's calibration. Lines after a
/// run's `Transfer/sec:` belong to a script's `done()` hook and are ignored.
pub struct AdapterHttpWrk;

impl Adaptable for AdapterHttpWrk {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let lines = input.lines().collect::<Vec<_>>();
        let mut results = LeafResults::default();
        let mut index = 0;
        while index < lines.len() {
            index = read_run(&lines, index, &mut results);
        }
        results.finish_with_skips(Adapter::HttpWrk)
    }
}

/// The run whose header starts at `index`, if one does, and the index to read on from: past the
/// run, or past its `Thread Stats` line when the run is cut short, so the next run's header is
/// still found.
fn read_run(lines: &[&str], index: usize, results: &mut LeafResults) -> usize {
    let Some((header, stats)) = Header::at(lines, index) else {
        return index + 1;
    };
    let mut rest = lines.get(stats + 1..).unwrap_or_default().iter().copied();
    if let Some(run) = Run::parse(&mut rest) {
        run.insert(&header, results);
        lines.len() - rest.len()
    } else {
        results.skip_benchmark();
        stats + 1
    }
}

const THREAD_STATS: &str = "  Thread Stats   Avg      Stdev     Max   +/- Stdev";
const DISTRIBUTION: &str = "  Latency Distribution";
const RECORDED: &str = "  Latency Distribution (HdrHistogram - Recorded Latency)";
const UNCORRECTED: &str = "  Latency Distribution (HdrHistogram - Uncorrected Latency (measured without taking delayed starts into account))";
const SPECTRUM: &str = "  Detailed Percentile spectrum:";

const DIRECTION: &str = "@direction";
const RECEIVED: &str = "received";
const VALUE: &str = "value";
const MEAN: &str = "mean";

/// What a run's header prints: its duration, URL, threads, and connections.
struct Header<'a> {
    duration: &'a str,
    url: &'a str,
    threads: u64,
    connections: u64,
}

impl<'a> Header<'a> {
    /// The header that starts at `index`, and the index of its `Thread Stats` line, or `None`
    /// when no `Thread Stats` line follows before another header or the end.
    fn at(lines: &[&'a str], index: usize) -> Option<(Self, usize)> {
        let header = Self::new(lines.get(index)?, lines.get(index + 1)?)?;
        for (stats, line) in lines.iter().enumerate().skip(index + 2) {
            if *line == THREAD_STATS {
                return Some((header, stats));
            }
            if lines
                .get(stats + 1)
                .is_some_and(|next| Self::new(line, next).is_some())
            {
                return None;
            }
        }
        None
    }

    /// `Running <duration> test @ <url>`, then `  <n> threads and <n> connections`.
    fn new(running: &'a str, threads: &str) -> Option<Self> {
        let (duration, url) = running.strip_prefix("Running ")?.split_once(" test @ ")?;
        let digits = duration.trim_end_matches(['s', 'm', 'h']);
        if digits.len() + 1 != duration.len() || !is_digits(digits) || url.is_empty() {
            return None;
        }
        let (threads, connections) = threads
            .strip_prefix("  ")?
            .strip_suffix(" connections")?
            .split_once(" threads and ")?;
        Some(Self {
            duration,
            url,
            threads: integer(threads)?,
            connections: integer(connections)?,
        })
    }

    /// The parameters of the run's variant, with `@direction` when `direction` is given.
    fn parameters(&self, direction: Option<&str>) -> Map<String, Value> {
        let mut parameters = Map::new();
        parameters.insert("@threads".to_owned(), self.threads.into());
        parameters.insert("@connections".to_owned(), self.connections.into());
        parameters.insert("@duration".to_owned(), self.duration.into());
        if let Some(direction) = direction {
            parameters.insert(DIRECTION.to_owned(), direction.into());
        }
        parameters
    }
}

/// The statistics of one run, after its `Thread Stats` line.
struct Run {
    /// Avg, Stdev, and Max, and the `+/- Stdev` cell, whose only use is to show an empty
    /// histogram.
    latency: [Finite; 4],
    recorded: Metrics,
    uncorrected: Metrics,
    requests: f64,
    errors: SocketErrors,
    error_responses: f64,
    rate: Finite,
    transfer: Finite,
}

impl Run {
    fn parse<'a, I: Iterator<Item = &'a str>>(lines: &mut I) -> Option<Self> {
        let [_, avg, stdev, max, within] = row(lines.next()?, "Latency")?;
        let latency = [time(avg)?, time(stdev)?, time(max)?, percent(within)?];
        row(lines.next()?, "Req/Sec")?;
        let mut line = lines.next()?;
        let mut recorded = Vec::new();
        if line == DISTRIBUTION {
            (recorded, line) = percentiles(lines)?;
        } else if line == RECORDED {
            // The recorded block's trailer repeats the `Thread Stats` row, rounded.
            (recorded, _) = spectrum(lines)?;
            line = next_line(lines)?;
        }
        let mut uncorrected = Vec::new();
        if line == UNCORRECTED {
            let (rows, trailer) = spectrum(lines)?;
            uncorrected = trailer.into_iter().chain(rows).collect();
            line = next_line(lines)?;
        }
        let (requests, rest) = line.strip_prefix("  ")?.split_once(" requests in ")?;
        rest.strip_suffix("B read")?;
        let requests = count(requests)?;
        line = lines.next()?;
        let errors = if let Some(errors) = line.strip_prefix("  Socket errors: ") {
            line = lines.next()?;
            SocketErrors::parse(errors)?
        } else {
            SocketErrors::default()
        };
        let error_responses =
            if let Some(responses) = line.strip_prefix("  Non-2xx or 3xx responses: ") {
                line = lines.next()?;
                count(responses)?
            } else {
                0.0
            };
        let rate = number(line.strip_prefix("Requests/sec:")?.trim_start())?;
        let transfer = bytes(lines.next()?.strip_prefix("Transfer/sec:")?.trim_start())?;
        Some(Self {
            latency,
            recorded,
            uncorrected,
            requests,
            errors,
            error_responses,
            rate,
            transfer,
        })
    }

    /// The run's variants, or none, counted as skipped, when its URL and settings repeat a run
    /// already read.
    fn insert(self, header: &Header<'_>, results: &mut LeafResults) {
        let Ok(name) = header.url.parse::<BenchmarkName>() else {
            results.skip_benchmark();
            return;
        };
        let (Some(run), Some(received)) = (
            parameter_set(header.parameters(None)),
            parameter_set(header.parameters(Some(RECEIVED))),
        ) else {
            results.skip_variant();
            return;
        };
        let Some(mut variant) = results.new_variant(name.clone(), run) else {
            // The repeat's received variant goes with it.
            results.skip_variant();
            return;
        };
        self.insert_run(&mut variant);
        insert(
            &mut results.variant(name, received),
            &BytesThroughput::name_id(),
            MEAN,
            self.transfer,
        );
    }

    fn insert_run(&self, variant: &mut Variant<'_>) {
        // wrk prints an empty histogram's statistics as 0 beside one that is not a number.
        if self.latency.iter().all(|cell| cell.get().is_some()) {
            let latency = Latency::name_id();
            for (metric, value) in ["mean", "std_dev", "max"].into_iter().zip(self.latency) {
                insert(variant, &latency, metric, value);
            }
            for (metric, value) in &self.recorded {
                insert(variant, &latency, metric, *value);
            }
            let uncorrected = UncorrectedLatency::name_id();
            for (metric, value) in &self.uncorrected {
                insert(variant, &uncorrected, metric, *value);
            }
        }
        let SocketErrors {
            connect,
            read,
            write,
            timeout,
        } = self.errors;
        for (measure, count) in [
            (Requests::name_id(), self.requests),
            (ConnectErrors::name_id(), connect),
            (ReadErrors::name_id(), read),
            (WriteErrors::name_id(), write),
            (Timeouts::name_id(), timeout),
            // wrk counts a response past the timeout as completed, so a timeout is no failure.
            (FailedRequests::name_id(), connect + read + write),
            (ErrorResponses::name_id(), self.error_responses),
        ] {
            variant.insert(measure, VALUE, count);
        }
        insert(variant, &Throughput::name_id(), MEAN, self.rate);
    }
}

/// The counts of `connect <n>, read <n>, write <n>, timeout <n>`, all 0 when wrk prints no line.
#[derive(Clone, Copy, Default)]
struct SocketErrors {
    connect: f64,
    read: f64,
    write: f64,
    timeout: f64,
}

impl SocketErrors {
    fn parse(errors: &str) -> Option<Self> {
        let rest = errors.strip_prefix("connect ")?;
        let (connect, rest) = rest.split_once(", read ")?;
        let (read, rest) = rest.split_once(", write ")?;
        let (write, timeout) = rest.split_once(", timeout ")?;
        Some(Self {
            connect: count(connect)?,
            read: count(read)?,
            write: count(write)?,
            timeout: count(timeout)?,
        })
    }
}

/// A finite value as `metric`, and nothing for one that is not a number, which wrk prints for a
/// statistic of no samples.
fn insert(variant: &mut Variant<'_>, measure: &MeasureNameId, metric: &str, value: Finite) {
    if let Some(value) = value.get() {
        variant.insert(measure.clone(), metric, value);
    }
}

/// A `Thread Stats` row's five cells, its label first.
fn row<'a>(line: &'a str, label: &str) -> Option<[&'a str; 5]> {
    let cells = line
        .strip_prefix("    ")?
        .split_whitespace()
        .collect::<Vec<_>>();
    let cells: [&str; 5] = cells.try_into().ok()?;
    (cells[0] == label).then_some(cells)
}

/// wrk's percentile rows, `<percent>%  <time>`, each by its metric name, with the line after
/// them.
fn percentiles<'a, I: Iterator<Item = &'a str>>(lines: &mut I) -> Option<(Metrics, &'a str)> {
    let mut rows = Vec::new();
    loop {
        let line = lines.next()?;
        let Some((percent, value)) = line.split_once('%') else {
            return Some((rows, line));
        };
        let percent = percent.trim_start();
        let name = percentile_name(percent)?;
        // wrk2's 100% row is the histogram's max, beside the row's own.
        if name != "max" {
            rows.push((name, time(value.trim())?));
        }
    }
}

/// wrk2's `HdrHistogram` block up to its closing rule: its percentile rows, and the `mean`,
/// `std_dev`, and `max` of the trailer after its detailed spectrum, which is skipped. Any other
/// line refuses the block, so a run cut short never reads into the next run's report.
fn spectrum<'a, I: Iterator<Item = &'a str>>(lines: &mut I) -> Option<(Metrics, Metrics)> {
    let (rows, line) = percentiles(lines)?;
    if !line.is_empty() || lines.next()? != SPECTRUM {
        return None;
    }
    let mut trailer = Vec::new();
    for line in lines {
        if let Some(cells) = line.strip_prefix("#[Mean") {
            let (mean, std_dev) = trailer_cells(cells, "StdDeviation")?;
            trailer.push((MEAN.to_owned(), millis(mean)?));
            trailer.push(("std_dev".to_owned(), millis(std_dev)?));
        } else if let Some(cells) = line.strip_prefix("#[Max") {
            let (max, _) = trailer_cells(cells, "Total count")?;
            trailer.push(("max".to_owned(), millis(max)?));
        } else if !line.is_empty() && line.bytes().all(|byte| byte == b'-') {
            return Some((rows, trailer));
        } else if !(line.is_empty()
            || line == SPECTRUM_COLUMNS
            || line.starts_with("#[")
            || is_spectrum_row(line))
        {
            return None;
        }
    }
    None
}

const SPECTRUM_COLUMNS: &str = "       Value   Percentile   TotalCount 1/(1-Percentile)";

/// `<value> <percentile> <count> <1/(1-percentile)>`, the last `inf` at the 100th percentile.
fn is_spectrum_row(line: &str) -> bool {
    let cells = line.split_whitespace().collect::<Vec<_>>();
    let [value, percentile, count, inverse] = cells.as_slice() else {
        return false;
    };
    is_decimal(value)
        && is_decimal(percentile)
        // wrk2 prints the 64-bit count with `%12d`, so from 2^31 it can print negative.
        && is_digits(count.strip_prefix('-').unwrap_or(count))
        && (is_decimal(inverse) || *inverse == "inf")
}

type Metrics = Vec<(String, Finite)>;

/// The 2 values of a trailer line after its first label, `= <value>, <label> = <value>]`.
fn trailer_cells<'a>(cells: &'a str, label: &str) -> Option<(&'a str, &'a str)> {
    let (first, second) = cells.strip_suffix(']')?.split_once(',')?;
    let second = second.trim_start().strip_prefix(label)?;
    Some((assigned(first)?, assigned(second)?))
}

fn assigned(cell: &str) -> Option<&str> {
    Some(cell.trim_start().strip_prefix('=')?.trim_start())
}

/// A trailer's time, in milliseconds.
fn millis(cell: &str) -> Option<Finite> {
    Some(number(cell)?.map(|time| time * Units::Milli.as_nanos()))
}

/// The next line that is not blank.
fn next_line<'a, I: Iterator<Item = &'a str>>(lines: &mut I) -> Option<&'a str> {
    lines.find(|line| !line.is_empty())
}

/// A time in nanoseconds, from `us`, `ms`, `s`, or `m`, the units wrk prints.
fn time(cell: &str) -> Option<Finite> {
    let (digits, nanos) = if let Some(digits) = cell.strip_suffix("us") {
        (digits, Units::Micro.as_nanos())
    } else if let Some(digits) = cell.strip_suffix("ms") {
        (digits, Units::Milli.as_nanos())
    } else if let Some(digits) = cell.strip_suffix('s') {
        (digits, Units::Sec.as_nanos())
    } else {
        (
            cell.strip_suffix('m')?,
            SECONDS_PER_MINUTE * Units::Sec.as_nanos(),
        )
    };
    Some(number(digits)?.map(|time| time * nanos))
}

const SECONDS_PER_MINUTE: f64 = 60.0;

/// A size in bytes, wrk's binary `K`, `M`, `G`, `T`, or no prefix, then `B`.
fn bytes(cell: &str) -> Option<Finite> {
    let cell = cell.strip_suffix('B')?;
    let (digits, power) = ["K", "M", "G", "T", "P"]
        .into_iter()
        .zip(1..)
        .find_map(|(prefix, power)| cell.strip_suffix(prefix).map(|digits| (digits, power)))
        .unwrap_or((cell, 0));
    Some(number(digits)?.map(|size| size * KIBI.powi(power)))
}

const KIBI: f64 = 1024.0;

fn percent(cell: &str) -> Option<Finite> {
    number(cell.strip_suffix('%')?)
}

/// A number as wrk prints it, `<digits>.<digits>`, or none for one that is not finite.
fn number(text: &str) -> Option<Finite> {
    if matches!(text, "nan" | "-nan" | "inf" | "-inf") {
        return Some(Finite::default());
    }
    is_decimal(text)
        .then(|| text.parse().ok())
        .flatten()
        .map(|number: f64| Finite::from(Some(number)))
}

fn is_decimal(text: &str) -> bool {
    text.split_once('.')
        .is_some_and(|(whole, fraction)| is_digits(whole) && is_digits(fraction))
}

fn integer(text: &str) -> Option<u64> {
    is_digits(text).then(|| text.parse().ok()).flatten()
}

fn count(text: &str) -> Option<f64> {
    is_digits(text).then(|| text.parse().ok()).flatten()
}

fn is_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
mod test_http_wrk {
    use bencher_json::project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{BytesThroughput, Latency, Throughput},
            http::{
                ConnectErrors, ErrorResponses, FailedRequests, ReadErrors, Requests, Timeouts,
                UncorrectedLatency, WriteErrors,
            },
        },
        report::Adapter,
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_variant, convert_v1, value, variants,
        },
    };

    use super::AdapterHttpWrk;

    const US: f64 = 1e3;
    const MS: f64 = 1e6;
    const S: f64 = 1e9;
    const MIN: f64 = 60.0 * S;
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;

    /// What one run reads to, by the URL and settings its header prints.
    struct Run {
        fixture: &'static str,
        url: &'static str,
        settings: (u64, u64, &'static str),
        latency: Option<Vec<(&'static str, f64)>>,
        uncorrected: Option<Vec<(&'static str, f64)>>,
        requests: f64,
        /// Connect, read, write, and timeout.
        errors: [f64; 4],
        error_responses: f64,
        rate: f64,
        transfer: f64,
    }

    fn parameters((threads, connections, duration): (u64, u64, &str), received: bool) -> String {
        let direction = if received {
            r#","@direction":"received""#
        } else {
            ""
        };
        format!(
            r#"{{"@connections":{connections}{direction},"@duration":"{duration}","@threads":{threads}}}"#
        )
    }

    fn counts(run: &Run) -> Vec<Expected> {
        let [connect, read, write, timeout] = run.errors;
        let count = |measure, count| (measure, vec![("value", count)]);
        let mut expected = vec![
            count(Requests::SLUG_STR, run.requests),
            count(ConnectErrors::SLUG_STR, connect),
            count(ReadErrors::SLUG_STR, read),
            count(WriteErrors::SLUG_STR, write),
            count(Timeouts::SLUG_STR, timeout),
            count(FailedRequests::SLUG_STR, connect + read + write),
            count(ErrorResponses::SLUG_STR, run.error_responses),
            (Throughput::SLUG_STR, vec![("mean", run.rate)]),
        ];
        if let Some(latency) = &run.latency {
            expected.push((Latency::SLUG_STR, latency.clone()));
        }
        if let Some(uncorrected) = &run.uncorrected {
            expected.push((UncorrectedLatency::SLUG_STR, uncorrected.clone()));
        }
        expected
    }

    fn assert_run(run: &Run) {
        let results = convert_v1(Adapter::HttpWrk, run.fixture);
        assert_eq!(
            results.resolved_adapter,
            Some(Adapter::HttpWrk),
            "{}",
            run.fixture
        );
        assert!(
            results.skipped.is_empty(),
            "{}: {:?}",
            run.fixture,
            results.skipped
        );
        assert_read(&results, run);
    }

    fn assert_read(results: &crate::AdapterResults, run: &Run) {
        assert_eq!(variants(results), 2, "{}", run.fixture);
        assert_variant(
            results,
            run.url,
            &parameters(run.settings, false),
            &counts(run),
        );
        assert_variant(
            results,
            run.url,
            &parameters(run.settings, true),
            &[(BytesThroughput::SLUG_STR, vec![("mean", run.transfer)])],
        );
    }

    fn summary(mean: f64, std_dev: f64, max: f64) -> Vec<(&'static str, f64)> {
        vec![("mean", mean), ("std_dev", std_dev), ("max", max)]
    }

    fn with(
        mut latency: Vec<(&'static str, f64)>,
        rows: &[(&'static str, f64)],
    ) -> Vec<(&'static str, f64)> {
        latency.extend_from_slice(rows);
        latency
    }

    fn hdr(rows: [f64; 7]) -> Vec<(&'static str, f64)> {
        ["median", "p75", "p90", "p99", "p99_9", "p99_99", "p99_999"]
            .into_iter()
            .zip(rows)
            .collect()
    }

    /// Kills a time or a size read in the wrong unit (`us`, `ms`, `s`, binary `K` and `M`, a bare
    /// `B`), a percentile misnamed, the `Req/Sec` row read, an absent error line left unwritten
    /// rather than 0, a summed `failed-requests` that adds the timeouts or leaves out a socket
    /// error, the `Non-2xx` line unread, the run's rate under any name but `mean`, the transfer
    /// rate on the run's own variant, and a parameter missing or misread.
    #[test]
    fn adapter_http_wrk_runs() {
        for run in [
            Run {
                fixture: "http/wrk/fast.txt",
                url: "http://localhost:8080/fast",
                settings: (2, 10, "3s"),
                latency: Some(summary(38.27 * US, 32.69 * US, 1.29 * MS)),
                uncorrected: None,
                requests: 815_015.0,
                errors: [0.0; 4],
                error_responses: 0.0,
                rate: 262_949.71,
                transfer: 29.84 * MIB,
            },
            Run {
                fixture: "http/wrk/slow_latency.txt",
                url: "http://localhost:8080/slow",
                settings: (2, 10, "3s"),
                latency: Some(with(
                    summary(50.38 * MS, 128.84 * US, 51.21 * MS),
                    &[
                        ("median", 50.36 * MS),
                        ("p75", 50.45 * MS),
                        ("p90", 50.54 * MS),
                        ("p99", 50.71 * MS),
                    ],
                )),
                uncorrected: None,
                requests: 590.0,
                errors: [0.0; 4],
                error_responses: 0.0,
                rate: 196.41,
                transfer: 3.09 * MIB,
            },
            Run {
                fixture: "http/wrk/error.txt",
                url: "http://localhost:8080/error",
                settings: (2, 10, "3s"),
                latency: Some(summary(38.04 * US, 31.49 * US, 1.98 * MS)),
                uncorrected: None,
                requests: 814_741.0,
                errors: [0.0; 4],
                error_responses: 814_741.0,
                rate: 262_878.10,
                transfer: 35.35 * MIB,
            },
            Run {
                fixture: "http/wrk/mixed_timeout.txt",
                url: "http://localhost:8080/mixed",
                settings: (2, 20, "6s"),
                latency: Some(with(
                    summary(42.30 * US, 42.96 * US, 381.0 * US),
                    &[
                        ("median", 30.0 * US),
                        ("p75", 44.0 * US),
                        ("p90", 71.0 * US),
                        ("p99", 254.0 * US),
                    ],
                )),
                uncorrected: None,
                requests: 1212.0,
                errors: [0.0, 63.0, 0.0, 40.0],
                error_responses: 248.0,
                rate: 201.75,
                transfer: 22.19 * KIB,
            },
            Run {
                fixture: "http/wrk/lua_post.txt",
                url: "http://localhost:8080/",
                settings: (2, 10, "3s"),
                latency: Some(summary(43.77 * US, 39.15 * US, 0.86 * MS)),
                uncorrected: None,
                requests: 748_358.0,
                errors: [0.0; 4],
                error_responses: 0.0,
                rate: 241_445.29,
                transfer: 29.70 * MIB,
            },
        ] {
            assert_run(&run);
        }
    }

    /// Kills wrk2's `-nan` in the `Req/Sec` row refused, and an `HdrHistogram` percentile read
    /// in the wrong unit or misnamed, such as 99.9 as `p99`.
    #[test]
    fn adapter_http_wrk_wrk2_runs() {
        for run in [
            Run {
                fixture: "http/wrk/wrk2_fast.txt",
                url: "http://localhost:8080/fast",
                settings: (2, 10, "5s"),
                latency: Some(summary(687.36 * US, 329.11 * US, 1.54 * MS)),
                uncorrected: None,
                requests: 9983.0,
                errors: [0.0; 4],
                error_responses: 0.0,
                rate: 1996.36,
                transfer: 232.0 * KIB,
            },
            Run {
                fixture: "http/wrk/wrk2_fast_latency.txt",
                url: "http://localhost:8080/fast",
                settings: (2, 10, "5s"),
                latency: Some(with(
                    summary(717.99 * US, 333.29 * US, 1.53 * MS),
                    &hdr([
                        711.0 * US,
                        1.00 * MS,
                        1.17 * MS,
                        1.34 * MS,
                        1.44 * MS,
                        1.53 * MS,
                        1.53 * MS,
                    ]),
                )),
                uncorrected: None,
                requests: 9984.0,
                errors: [0.0; 4],
                error_responses: 0.0,
                rate: 1996.46,
                transfer: 232.01 * KIB,
            },
            wrk2_mixed(),
        ] {
            assert_run(&run);
        }
    }

    fn wrk2_mixed() -> Run {
        Run {
            fixture: "http/wrk/wrk2_mixed_latency.txt",
            url: "http://localhost:8080/mixed",
            settings: (2, 20, "6s"),
            latency: Some(with(
                summary(797.35 * MS, 1.26 * S, 3.00 * S),
                &hdr([
                    0.97 * MS,
                    2.44 * S,
                    2.92 * S,
                    3.00 * S,
                    3.00 * S,
                    3.00 * S,
                    3.00 * S,
                ]),
            )),
            uncorrected: None,
            requests: 229.0,
            errors: [0.0, 0.0, 0.0, 50.0],
            error_responses: 53.0,
            rate: 37.85,
            transfer: 4.14 * KIB,
        }
    }

    /// Kills wrk2's 100% row read as `max` over the `Thread Stats` max or the uncorrected
    /// trailer's, an uncorrected block written to `latency`, its trailer left unread or read in
    /// the wrong unit, the recorded block's trailer read over the `Thread Stats` row, the spectrum
    /// read, and wrk2's calibration lines before `Thread Stats` refused.
    #[test]
    fn adapter_http_wrk_uncorrected() {
        for run in [
            wrk2_slow_u(),
            Run {
                fixture: "http/wrk/wrk2_long_latency_u.txt",
                url: "http://localhost:8080/fast",
                settings: (2, 10, "15s"),
                latency: Some(with(
                    summary(750.68 * US, 352.90 * US, 1.66 * MS),
                    &hdr([
                        733.0 * US,
                        1.04 * MS,
                        1.22 * MS,
                        1.46 * MS,
                        1.56 * MS,
                        1.66 * MS,
                        1.66 * MS,
                    ]),
                )),
                uncorrected: Some(with(
                    summary(0.054 * MS, 0.025 * MS, 0.253 * MS),
                    &hdr([
                        52.0 * US,
                        70.0 * US,
                        87.0 * US,
                        126.0 * US,
                        202.0 * US,
                        253.0 * US,
                        253.0 * US,
                    ]),
                )),
                requests: 14_994.0,
                errors: [0.0; 4],
                error_responses: 0.0,
                rate: 999.51,
                transfer: 116.15 * KIB,
            },
        ] {
            assert_run(&run);
        }
    }

    fn wrk2_slow_u() -> Run {
        Run {
            fixture: "http/wrk/wrk2_slow_u_latency.txt",
            url: "http://localhost:8080/slow",
            settings: (2, 10, "5s"),
            latency: Some(with(
                summary(51.65 * MS, 423.29 * US, 53.28 * MS),
                &hdr([
                    51.68 * MS,
                    51.97 * MS,
                    52.19 * MS,
                    52.54 * MS,
                    53.28 * MS,
                    53.31 * MS,
                    53.31 * MS,
                ]),
            )),
            uncorrected: Some(with(
                summary(50.875 * MS, 0.238 * MS, 52.704 * MS),
                &hdr([
                    50.91 * MS,
                    50.97 * MS,
                    51.07 * MS,
                    51.23 * MS,
                    52.74 * MS,
                    52.74 * MS,
                    52.74 * MS,
                ]),
            )),
            requests: 748.0,
            errors: [0.0; 4],
            error_responses: 0.0,
            rate: 149.59,
            transfer: 2.36 * MIB,
        }
    }

    /// Kills a latency in minutes read in any other unit: wrk prints a time of 51 s or more in
    /// minutes.
    #[test]
    fn adapter_http_wrk_minutes() {
        let minutes = 0.92 * MIN;
        assert_run(&Run {
            fixture: "http/wrk/hang_minutes.txt",
            url: "http://localhost:8080/hang",
            settings: (1, 2, "1m"),
            latency: Some(with(
                summary(minutes, 89.80 * US, minutes),
                &[
                    ("median", minutes),
                    ("p75", minutes),
                    ("p90", minutes),
                    ("p99", minutes),
                ],
            )),
            uncorrected: None,
            requests: 2.0,
            errors: [0.0; 4],
            error_responses: 0.0,
            rate: 0.03,
            transfer: 1.96,
        });
    }

    /// Kills the zeros wrk and wrk2 print for an empty histogram stored as latency, keyed on 0
    /// requests alone (wrk counts a response past its timeout as a request but records no
    /// latency for it), or counted as skipped, and a `-nan` refused.
    #[test]
    fn adapter_http_wrk_empty_histogram() {
        for run in [
            Run {
                fixture: "http/wrk/drop.txt",
                url: "http://localhost:8080/drop",
                settings: (2, 10, "3s"),
                latency: None,
                uncorrected: None,
                requests: 0.0,
                errors: [0.0, 194_329.0, 0.0, 0.0],
                error_responses: 0.0,
                rate: 0.0,
                transfer: 0.0,
            },
            Run {
                fixture: "http/wrk/late_timeout.txt",
                url: "http://localhost:8080/hang",
                settings: (1, 2, "4s"),
                latency: None,
                uncorrected: None,
                requests: 4.0,
                errors: [0.0, 0.0, 0.0, 4.0],
                error_responses: 0.0,
                rate: 1.0,
                transfer: 58.92,
            },
            Run {
                fixture: "http/wrk/wrk2_drop.txt",
                url: "http://localhost:8080/drop",
                settings: (2, 10, "3s"),
                latency: None,
                uncorrected: None,
                requests: 0.0,
                errors: [0.0, 0.0, 0.0, 20.0],
                error_responses: 0.0,
                rate: 0.0,
                transfer: 0.0,
            },
        ] {
            assert_run(&run);
        }
    }

    /// Kills a script's `done()` output after `Transfer/sec:` read or counted as skipped.
    #[test]
    fn adapter_http_wrk_done_hook() {
        assert_run(&Run {
            fixture: "http/wrk/lua_done.txt",
            url: "http://localhost:8080/fast",
            settings: (2, 10, "3s"),
            latency: Some(with(
                summary(37.97 * US, 30.34 * US, 813.0 * US),
                &[
                    ("median", 30.0 * US),
                    ("p75", 37.0 * US),
                    ("p90", 53.0 * US),
                    ("p99", 188.0 * US),
                ],
            )),
            uncorrected: None,
            requests: 825_692.0,
            errors: [0.0; 4],
            error_responses: 0.0,
            rate: 266_395.83,
            transfer: 30.23 * MIB,
        });
    }

    /// Kills a socket error read into another kind's measure, and a summed `failed-requests`
    /// that leaves out one kind, which no capture shows since none has every kind above 0.
    #[test]
    fn adapter_http_wrk_socket_errors() {
        let mixed = fixture("http/wrk/mixed_timeout.txt").replace(
            "connect 0, read 63, write 0, timeout 40",
            "connect 3, read 5, write 7, timeout 11",
        );
        let results = parse(&mixed).expect("no results");
        let parameters = parameters((2, 20, "6s"), false);
        for (measure, expected) in [
            (ConnectErrors::SLUG_STR, 3.0),
            (ReadErrors::SLUG_STR, 5.0),
            (WriteErrors::SLUG_STR, 7.0),
            (Timeouts::SLUG_STR, 11.0),
            (FailedRequests::SLUG_STR, 15.0),
        ] {
            assert_eq!(
                value(
                    &results,
                    "http://localhost:8080/mixed",
                    &parameters,
                    measure,
                    "value"
                ),
                Some(expected),
                "{measure}"
            );
        }
    }

    /// Kills a spectrum row refused for a negative count: wrk2 can print a count of 2^31 or more
    /// negative, which no capture shows since it takes 2^31 requests.
    #[test]
    fn adapter_http_wrk_negative_count() {
        let run = wrk2_slow_u();
        let wrapped = fixture(run.fixture).replace(
            "53.311     1.000000          746",
            "53.311     1.000000  -2147482902",
        );
        let results = parse(&wrapped).expect("no results");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_read(&results, &run);
    }

    fn fixture(suffix: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/{suffix}"))
            .expect("Failed to read a fixture")
    }

    fn parse(input: &str) -> Option<crate::AdapterResults> {
        AdapterHttpWrk::parse(input, Settings::default())
    }

    /// Kills a repeated run written over the first or skipped uncounted, settings left out of
    /// the variant so that runs at different connections collide, and a leaf that stops after
    /// its first run.
    #[test]
    fn adapter_http_wrk_repeats() {
        let fast = fixture("http/wrk/fast.txt");
        let again = fast.replace("262949.71", "1.00");
        let results = parse(&format!("{fast}{again}")).expect("no results");
        assert_eq!(results.skipped.variants, 2);
        assert_eq!(variants(&results), 2);
        let throughput = value(
            &results,
            "http://localhost:8080/fast",
            &parameters((2, 10, "3s"), false),
            Throughput::SLUG_STR,
            "mean",
        );
        assert_eq!(throughput, Some(262_949.71));

        let wider = fast.replace("10 connections", "20 connections");
        let results = parse(&format!("{fast}{wider}")).expect("no results");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_eq!(results.inner.len(), 1);
        assert_eq!(variants(&results), 4);
    }

    /// Kills a gate that claims a `Running` line and threads line with no `Thread Stats` header,
    /// the two lines apart, autocannon's `Running` line and connections line, or a header that a
    /// later run's `Thread Stats` completes, and one that refuses a script's print while the
    /// threads run.
    #[test]
    fn adapter_http_wrk_gate() {
        let fast = fixture("http/wrk/fast.txt");
        let lines = fast.lines().collect::<Vec<_>>();
        let header = lines[..2].join("\n");
        assert_eq!(parse(&header), None);
        let apart = [lines[0], "", &lines[1..].join("\n")].join("\n");
        assert_eq!(parse(&apart), None);
        for autocannon in [
            "http/autocannon/slow_latency_codes.txt",
            "http/autocannon/mixed_text_9_0_0.txt",
        ] {
            assert_eq!(parse(&fixture(autocannon)), None, "{autocannon}");
        }

        let cut = format!("{header}\n{}", fast.replace("/fast", "/other"));
        let results = parse(&cut).expect("no results");
        assert_eq!(
            results
                .inner
                .keys()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec!["http://localhost:8080/other".to_owned()]
        );

        let printed = [&header, "a script's own print", &lines[2..].join("\n")].join("\n");
        assert_eq!(parse(&printed), parse(&fast));
    }

    /// Kills a run cut short that is written in part, refused so that `magic` falls through to
    /// `console.time`, or that takes the next run's header with it, and a line wrk does not print
    /// inside a run read past.
    #[test]
    fn adapter_http_wrk_cut_short() {
        let long = fixture("http/wrk/wrk2_long_latency_u.txt");
        let cut = long.replace("Transfer/sec:    116.15KB", "");
        let results = Adapter::Magic
            .convert(&cut, Settings::new(None, bencher_json::BmfVersion::V1))
            .expect("no results");
        assert_eq!(results.resolved_adapter, Some(Adapter::HttpWrk));
        assert_eq!(results.skipped.benchmarks, 1);
        assert!(results.inner.is_empty());

        let fast = fixture("http/wrk/fast.txt");
        let stray = fast.replace("Requests/sec:", "  stray\nRequests/sec:");
        let results = parse(&format!("{stray}{fast}")).expect("no results");
        assert_eq!(results.skipped.benchmarks, 1);
        assert_eq!(variants(&results), 2);

        let (stopped, _) = fast.split_once("Requests/sec:").expect("no rate");
        let slow = fixture("http/wrk/slow_latency.txt");
        let results = parse(&format!("{stopped}{slow}")).expect("no results");
        assert_eq!(results.skipped.benchmarks, 1);
        assert_eq!(
            results
                .inner
                .keys()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec!["http://localhost:8080/slow".to_owned()]
        );
    }

    /// Kills a wrk2 run cut inside its recorded spectrum that reads on to the next run's closing
    /// rule and takes that run's numbers, or that is skipped uncounted.
    #[test]
    fn adapter_http_wrk_cut_in_spectrum() {
        let fast = fixture("http/wrk/wrk2_fast_latency.txt");
        let (cut, _) = fast.split_once("#[Mean").expect("no trailer");
        for run in [wrk2_slow_u(), wrk2_mixed()] {
            let input = format!("{cut}{}", fixture(run.fixture));
            let results = parse(&input).expect("no results");
            assert_eq!(results.skipped.benchmarks, 1, "{}", run.fixture);
            assert_eq!(results.skipped.variants, 0, "{}", run.fixture);
            assert_eq!(
                results
                    .inner
                    .keys()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                vec![run.url.to_owned()]
            );
            assert_read(&results, &run);
        }
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_http_wrk_ignores_average() {
        assert_ignores_average(Adapter::HttpWrk, "http/wrk/wrk2_slow_u_latency.txt");
    }
}
