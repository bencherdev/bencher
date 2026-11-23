use bencher_json::{
    BenchmarkName, ParameterSet,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Latency, Throughput},
            http::{FailedRequests, Requests},
        },
        report::Adapter,
    },
};
use serde_json::{Map, Value};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        finite::Finite,
        leaf::{LeafResults, Variant, parameter_set},
        percentile::percentile_name,
    },
    results::adapter_results::AdapterResults,
};

/// Gatling's console (`ConsoleSummary.scala`, `ConsoleTemplate.scala`, and
/// `ConsoleStatsFormat.scala` at v3.16.1), in the table Gatling prints from 3.13.4 on.
///
/// A run is one benchmark that the server names by the run's benchmark prefix, else `gatling`:
/// its Global Information, which Gatling prints once it builds its reports, or with reports off
/// the counts of its last Requests table's `Global` row. Each request of that table is a
/// benchmark named by its group path. Maven, Gradle, and sbt print Gatling's lines as they are
/// among lines of their own, so every line outside these blocks is ignored. An input that shows
/// a second simulation is refused, since its blocks would name the same benchmarks.
pub struct AdapterHttpGatling;

impl Adaptable for AdapterHttpGatling {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let lines = input.lines().collect::<Vec<_>>();
        let (mut table, mut global, mut before) = (None, None, None);
        for (index, line) in lines.iter().enumerate() {
            if titled_with_columns(line, REQUESTS) {
                if global.is_some() {
                    return None;
                }
                // One simulation's time and count only grow (`ConsoleDataWriter.scala:86`,
                // `:129-137`), and the line that starts the next is hidden at Gatling's default
                // log level (`Runner.scala:147`).
                if let Some(progress) = progress(&lines, index) {
                    if before.is_some_and(|before: Progress| {
                        progress.seconds < before.seconds || progress.requests < before.requests
                    }) {
                        return None;
                    }
                    before = Some(progress);
                }
                table = Some(index);
            } else if titled_with_columns(line, GLOBAL_INFORMATION) {
                if global.is_some() {
                    return None;
                }
                global = Some(index);
            }
        }
        let mut results = LeafResults::default();
        let mut run = None;
        if let Some(index) = table {
            if let Some(Table {
                global: row,
                requests,
            }) = read_table(&lines, index)
            {
                insert_requests(&mut results, requests);
                run = Some(Run {
                    columns: <[Column; 3]>::default(),
                    requests: row.total.value,
                    failed: row.failed.value,
                });
            } else {
                results.skip_benchmark();
            }
        }
        if let Some(index) = global {
            run = read_run(&lines, index);
            if run.is_none() {
                results.skip_benchmark();
            }
        }
        if let Some(run) = run {
            run.insert(&mut results);
        }
        results.finish_with_skips(Adapter::HttpGatling)
    }
}

/// The name of the run's benchmark when the run sets no prefix.
const GATLING: &str = "gatling";
const REQUESTS: &str = "Requests";
const GLOBAL_INFORMATION: &str = "Global Information";
/// The label of the run's own row in a Requests table.
const GLOBAL: &str = "Global";
const ERRORS: &str = "Errors";
const VALUE: &str = "value";
const MEAN: &str = "mean";
/// The cell Gatling prints for a statistic of a column with no request.
const ABSENT: &str = "-";
/// The cell Gatling prints for a rate over a run inside one millisecond, which lasts 0 seconds
/// (`ResultsHolder.scala:30`, `GeneralStatsBuffers.scala:84`).
const INFINITY: &str = "∞";
/// `ConsoleStatsFormat.scala:29`.
const CONSOLE_WIDTH: usize = 120;
const COLUMNS: &str = "|---Total---|-----OK----|----KO----";

/// How far a simulation is at one Requests table.
#[derive(Clone, Copy)]
struct Progress {
    seconds: u64,
    requests: u64,
}

/// The seconds elapsed and the run's request count of the table whose title is at `index`.
fn progress(lines: &[&str], index: usize) -> Option<Progress> {
    let seconds = elapsed(lines.get(index.checked_sub(1)?)?)?;
    let global = counts(lines.get(index + 1)?)?;
    (global.name == GLOBAL).then_some(Progress {
        seconds,
        requests: global.total.whole,
    })
}

/// One row of a Requests table.
struct Request<'a> {
    name: &'a str,
    total: Count,
    failed: Count,
}

/// The rows of the table whose title is at `index`, or `None` when its rule, its time, or a row
/// is not the line Gatling prints there (`ConsoleSummary.scala:101-110`), or when its request
/// rows do not sum to the run's.
fn read_table<'a>(lines: &[&'a str], index: usize) -> Option<Table<'a>> {
    let opening = lines.get(index.checked_sub(2)?..index)?;
    let [opening_rule, time] = opening else {
        return None;
    };
    if !rule(opening_rule) {
        return None;
    }
    elapsed(time)?;
    let mut rows = lines.get(index + 1..)?.iter();
    let run = counts(rows.next()?)?;
    if run.name != GLOBAL {
        return None;
    }
    let mut requests = Vec::<Request<'a>>::new();
    for row in rows {
        if row.is_empty() || titled(row, ERRORS) {
            // Each response counts once for the run and once for its request
            // (`ConsoleDataWriter.scala:121-138`), so a table cut at a row sums short.
            let summed = |count: fn(&Request<'_>) -> u64| {
                requests
                    .iter()
                    .try_fold(0u64, |sum, request| sum.checked_add(count(request)))
            };
            let whole = requests.is_empty()
                || (summed(|request| request.total.whole) == Some(run.total.whole)
                    && summed(|request| request.failed.whole) == Some(run.failed.whole));
            return whole.then_some(Table {
                global: run,
                requests,
            });
        }
        requests.push(counts(row)?);
    }
    None
}

/// The run's own row of a Requests table and its request rows.
struct Table<'a> {
    global: Request<'a>,
    requests: Vec<Request<'a>>,
}

/// Each request as a benchmark with its total and KO counts. A request named as one before it,
/// as two long names that Gatling truncates alike, is skipped and counted.
fn insert_requests(results: &mut LeafResults, requests: Vec<Request<'_>>) {
    for Request {
        name,
        total,
        failed,
    } in requests
    {
        let Ok(benchmark) = name.parse::<BenchmarkName>() else {
            results.skip_benchmark();
            continue;
        };
        if let Some(mut variant) = results.new_variant(benchmark, ParameterSet::default()) {
            variant.insert(Requests::name_id(), VALUE, total.value);
            variant.insert(FailedRequests::name_id(), VALUE, failed.value);
        }
    }
}

/// A request row, `> <name> | <total> | <OK> | <KO>`, whose total is its OK and KO counts summed.
fn counts(line: &str) -> Option<Request<'_>> {
    let (name, [total, ok, ko]) = row(line)?;
    let (total, ok, ko) = (count(total)?, count(ok)?, count(ko)?);
    (ok.whole.checked_add(ko.whole)? == total.whole).then_some(Request {
        name,
        total,
        failed: ko,
    })
}

/// The label and the Total, OK, and KO cells of `> <label> | <Total> | <OK> | <KO>`, the label
/// padded and each cell aligned to the right (`ConsoleTemplate.scala:28-31`).
fn row(line: &str) -> Option<(&str, [&str; 3])> {
    let mut parts = line.strip_prefix("> ")?.rsplitn(4, " | ");
    let ko = parts.next()?;
    let ok = parts.next()?;
    let total = parts.next()?;
    let label = parts.next()?.trim_end_matches(' ');
    Some((
        label,
        [total, ok, ko].map(|cell| cell.trim_start_matches(' ')),
    ))
}

/// The run's statistics in the Global Information whose title is at `index`, or `None` when any
/// line from its rule to the end of its ranges is not the one Gatling prints there
/// (`ConsoleTemplate.scala:49-74`). The error rows after the ranges are the user's own text.
fn read_run(lines: &[&str], index: usize) -> Option<Run> {
    if !rule(lines.get(index.checked_sub(1)?)?) {
        return None;
    }
    let mut lines = lines.get(index + 1..)?.iter().copied();
    let [total, ok, ko] = labelled(lines.next()?, "request count")?;
    let (total, ok, ko) = (run_count(total)?, run_count(ok)?, run_count(ko)?);
    if ok.whole.checked_add(ko.whole)? != total.whole {
        return None;
    }
    let mut columns = <[Column; 3]>::default();
    for (label, metric) in [
        ("min response time (ms)", "min"),
        ("max response time (ms)", "max"),
        ("mean response time (ms)", MEAN),
        ("response time std deviation (ms)", "std_dev"),
    ] {
        push_times(&mut columns, metric, labelled(lines.next()?, label)?)?;
    }
    for _ in 0..4 {
        let (metric, cells) = percentile(lines.next()?)?;
        if let Some(metric) = metric {
            push_times(&mut columns, &metric, cells)?;
        }
    }
    let [total_rate, ok_rate, ko_rate] = labelled(lines.next()?, "mean throughput (rps)")?;
    for (column, cell) in columns.iter_mut().zip([total_rate, ok_rate, ko_rate]) {
        column.rate = rate(cell)?.get();
    }
    let title = lines.next()?;
    let ranges = [lines.next()?, lines.next()?, lines.next()?, lines.next()?];
    if !titled(title, "Response Time Distribution") || !response_ranges(ranges)? {
        return None;
    }
    let next = lines.next()?;
    (rule(next) || titled(next, ERRORS)).then_some(Run {
        columns,
        requests: total.value,
        failed: ko.value,
    })
}

/// What one run reports, each metric finite, with no column statistics when it is read from a
/// table's `Global` row.
struct Run {
    /// The Total, OK, and KO columns.
    columns: [Column; 3],
    requests: f64,
    failed: f64,
}

/// The latency metrics and the rate of one column.
#[derive(Default)]
struct Column {
    latency: Vec<(String, f64)>,
    rate: Option<f64>,
}

impl Run {
    /// The Total column and the counts as the run's benchmark, and the OK and KO columns as its
    /// `@responses` variants, each only when the column holds a request.
    fn insert(self, results: &mut LeafResults) {
        let Self {
            columns: [total, ok, ko],
            requests,
            failed,
        } = self;
        let mut run = results.nameless_variant(GATLING, ParameterSet::default());
        total.insert(&mut run);
        run.insert(Requests::name_id(), VALUE, requests);
        run.insert(FailedRequests::name_id(), VALUE, failed);
        for (responses, column) in [("ok", ok), ("ko", ko)] {
            if column.latency.is_empty() && column.rate.is_none() {
                continue;
            }
            let Some(parameters) = parameter_set(Map::from_iter([(
                "@responses".to_owned(),
                Value::from(responses),
            )])) else {
                results.skip_variant();
                continue;
            };
            column.insert(&mut results.nameless_variant(GATLING, parameters));
        }
    }
}

impl Column {
    fn insert(self, variant: &mut Variant<'_>) {
        for (metric, value) in self.latency {
            variant.insert(Latency::name_id(), &metric, value);
        }
        if let Some(rate) = self.rate {
            variant.insert(Throughput::name_id(), MEAN, rate);
        }
    }
}

/// Each column's time in `cells` as `metric`, for the columns that hold one.
fn push_times(columns: &mut [Column; 3], metric: &str, cells: [&str; 3]) -> Option<()> {
    let [total, ok, ko] = cells;
    for (column, time) in columns
        .iter_mut()
        .zip([millis(total)?, millis(ok)?, millis(ko)?])
    {
        if let Some(time) = time.get() {
            column.latency.push((metric.to_owned(), time));
        }
    }
    Some(())
}

/// The cells of a row whose label is `label`.
fn labelled<'a>(line: &'a str, label: &str) -> Option<[&'a str; 3]> {
    let (printed, cells) = row(line)?;
    (printed == label).then_some(cells)
}

/// The cells of `> response time <level><ordinal> percentile (ms) | ...` with the metric its
/// level names, or no metric at a level of 0 or 100, which yields to Gatling's exact `min` or
/// `max`.
fn percentile(line: &str) -> Option<(Option<String>, [&str; 3])> {
    let (label, cells) = row(line)?;
    let rank = label
        .strip_prefix("response time ")?
        .strip_suffix(" percentile (ms)")?;
    let level = ["st", "nd", "rd", "th"]
        .into_iter()
        .find_map(|ordinal| rank.strip_suffix(ordinal))?;
    let metric = percentile_name(&decimal(level)?)?;
    Some((
        (metric != "min" && metric != "max").then_some(metric),
        cells,
    ))
}

/// Whether the 4 rows are the response time ranges, `OK: t < <lower> ms`, `OK: <lower> ms <= t <
/// <higher> ms`, `OK: t >= <higher> ms`, and `KO`, each with its count and percentage
/// (`ConsoleTemplate.scala:33-34`, `:67-70`), or `None` when a row is not a range row.
fn response_ranges([below, between, above, ko]: [&str; 4]) -> Option<bool> {
    let lower = range(below)?
        .strip_prefix("OK: t < ")?
        .strip_suffix(" ms")?;
    let higher = range(above)?
        .strip_prefix("OK: t >= ")?
        .strip_suffix(" ms")?;
    Some(
        digits(lower)
            && digits(higher)
            && range(between)? == format!("OK: {lower} ms <= t < {higher} ms")
            && range(ko)? == "KO",
    )
}

/// The label of `> <label> <count> (<percentage>%)`.
fn range(line: &str) -> Option<&str> {
    let (rest, percentage) = line.strip_prefix("> ")?.rsplit_once(' ')?;
    let (label, count) = rest.trim_end_matches(' ').rsplit_once(' ')?;
    whole(count)?;
    decimal(percentage.strip_prefix('(')?.strip_suffix("%)")?)?;
    Some(label.trim_end_matches(' '))
}

/// Whether `line` is a rule, the 120 `=` that open and close each block.
fn rule(line: &str) -> bool {
    line.len() == CONSOLE_WIDTH && line.bytes().all(|byte| byte == b'=')
}

/// Whether `line` is `---- <title> ` padded with `-` to the console's width
/// (`ConsoleSummary.scala:34-35`).
fn titled(line: &str, title: &str) -> bool {
    line.len() == CONSOLE_WIDTH
        && line
            .strip_prefix("---- ")
            .and_then(|rest| rest.strip_prefix(title))
            .and_then(|rest| rest.strip_prefix(' '))
            .is_some_and(|rest| rest.bytes().all(|byte| byte == b'-'))
}

/// Whether `line` is `---- <title> ` padded with `-` and followed by the Total, OK, and KO
/// column titles (`ConsoleStatsFormat.scala:68-71`).
fn titled_with_columns(line: &str, title: &str) -> bool {
    line.len() == CONSOLE_WIDTH
        && line
            .strip_suffix(COLUMNS)
            .and_then(|rest| rest.strip_prefix("---- "))
            .and_then(|rest| rest.strip_prefix(title))
            .and_then(|rest| rest.strip_prefix(' '))
            .is_some_and(|rest| rest.bytes().all(|byte| byte == b'-'))
}

/// The seconds of a summary's time, `<yyyy-MM-dd> <HH:mm:ss> <zone> <seconds>s elapsed`, whose
/// zone the JVM's language names, in some languages with a space (`ConsoleDataWriter.scala:75`,
/// `ConsoleSummary.scala:105`).
fn elapsed(line: &str) -> Option<u64> {
    let (stamp, seconds) = line.strip_suffix("s elapsed")?.rsplit_once(' ')?;
    let (date, rest) = stamp.split_at_checked(10)?;
    let (time, zone) = rest.strip_prefix(' ')?.split_at_checked(8)?;
    let zone = zone.strip_prefix(' ')?.trim_end_matches(' ');
    if !digits(seconds)
        || !shaped(date, "dddd-dd-dd")
        || !shaped(time, "dd:dd:dd")
        || zone.is_empty()
    {
        return None;
    }
    seconds.parse().ok()
}

/// Whether `text` has a digit where `shape` has `d` and the same byte elsewhere.
fn shaped(text: &str, shape: &str) -> bool {
    text.len() == shape.len()
        && text.bytes().zip(shape.bytes()).all(|(byte, expected)| {
            if expected == b'd' {
                byte.is_ascii_digit()
            } else {
                byte == expected
            }
        })
}

/// A count, as a whole number to sum and as its value.
#[derive(Clone, Copy)]
struct Count {
    whole: u64,
    value: f64,
}

/// A count in a request row.
fn count(cell: &str) -> Option<Count> {
    let digits = whole(cell)?;
    Some(Count {
        whole: digits.parse().ok()?,
        value: digits.parse().ok()?,
    })
}

/// A count in the Global Information, where `-` is a column with no request, so 0.
fn run_count(cell: &str) -> Option<Count> {
    if cell == ABSENT {
        Some(Count {
            whole: 0,
            value: 0.0,
        })
    } else {
        count(cell)
    }
}

/// A time in ns from the whole ms Gatling prints (`GeneralStats.scala:19`), scaled by its digits
/// so that it is exact, or no time for `-`.
fn millis(cell: &str) -> Option<Finite> {
    if cell == ABSENT {
        return Some(Finite::default());
    }
    Some(Finite::from(Some(
        format!("{}000000", whole(cell)?).parse().ok()?,
    )))
}

/// A rate, or no rate for `-` or `∞`.
fn rate(cell: &str) -> Option<Finite> {
    if cell == ABSENT || cell == INFINITY {
        return Some(Finite::default());
    }
    Some(Finite::from(Some(decimal(cell)?.parse().ok()?)))
}

/// The digits of a number Gatling prints with `###,###.##` in English (`NumberHelper.scala:26`
/// in gatling-shared-util 0.1.0): a whole number, then a fraction of 1 or 2 digits with no
/// trailing zero.
fn decimal(cell: &str) -> Option<String> {
    let Some((whole_part, fraction)) = cell.split_once('.') else {
        return whole(cell);
    };
    let mut number = whole(whole_part)?;
    let valid = matches!(fraction.len(), 1 | 2) && digits(fraction) && !fraction.ends_with('0');
    valid.then(move || {
        number.push('.');
        number.push_str(fraction);
        number
    })
}

/// The digits of a whole number Gatling prints with `###,###`: groups of 3 joined by commas,
/// after a first group of 1 to 3, with no leading zero.
fn whole(cell: &str) -> Option<String> {
    let mut groups = cell.split(',');
    let first = groups.next()?;
    if !matches!(first.len(), 1..=3) || !digits(first) {
        return None;
    }
    let mut number = first.to_owned();
    for group in groups {
        if group.len() != 3 || !digits(group) {
            return None;
        }
        number.push_str(group);
    }
    (number == "0" || !number.starts_with('0')).then_some(number)
}

/// Whether `text` is one or more ASCII digits.
fn digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
mod test_http_gatling {
    use bencher_json::{
        BmfVersion, ParameterSet,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{Latency, Throughput},
                http::{FailedRequests, Requests},
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::{
            http::gatling::AdapterHttpGatling,
            test_util::{Expected, assert_ignores_average, assert_variant, convert_v1, value},
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const LEAF: Adapter = Adapter::HttpGatling;
    const OK: &str = r#"{"@responses":"ok"}"#;
    const KO: &str = r#"{"@responses":"ko"}"#;

    fn named(suffix: &str) -> AdapterResults {
        let mut results = convert_v1(LEAF, &format!("http/gatling/{suffix}"));
        results.name_nameless(None);
        results
    }

    fn parse(input: &str) -> Option<AdapterResults> {
        AdapterHttpGatling::parse(input, Settings::new(None, BmfVersion::V1))
    }

    fn parsed(input: &str) -> AdapterResults {
        let mut results = parse(input).expect("Failed to parse the console");
        results.name_nameless(None);
        results
    }

    fn fixture(suffix: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/http/gatling/{suffix}"))
            .expect("Failed to read a fixture")
    }

    /// The fixture's lines, the last that starts with `start` replaced by `line`.
    fn replaced(suffix: &str, start: &str, line: &str) -> String {
        let mut lines = fixture(suffix)
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let index = lines
            .iter()
            .rposition(|candidate| candidate.starts_with(start))
            .unwrap_or_else(|| panic!("Failed to find {start} in {suffix}"));
        if let Some(found) = lines.get_mut(index) {
            line.clone_into(found);
        }
        lines.join("\n")
    }

    /// `latency` from the minimum, maximum, mean, standard deviation, and the median, `p75`,
    /// `p95`, and `p99`, each in ms.
    fn latency([min, max, mean, std_dev]: [f64; 4], [p50, p75, p95, p99]: [f64; 4]) -> Expected {
        (
            Latency::SLUG_STR,
            [
                ("min", min),
                ("max", max),
                ("mean", mean),
                ("std_dev", std_dev),
                ("median", p50),
                ("p75", p75),
                ("p95", p95),
                ("p99", p99),
            ]
            .into_iter()
            .map(|(metric, millis)| (metric, millis * 1e6))
            .collect(),
        )
    }

    fn throughput(rate: f64) -> Expected {
        (Throughput::SLUG_STR, vec![("mean", rate)])
    }

    fn count(measure: &'static str, count: f64) -> Expected {
        (measure, vec![("value", count)])
    }

    /// A request's total and KO counts.
    fn request(total: f64, ko: f64) -> [Expected; 2] {
        [
            count(Requests::SLUG_STR, total),
            count(FailedRequests::SLUG_STR, ko),
        ]
    }

    /// The benchmarks the results name, sorted.
    fn benchmarks(results: &AdapterResults) -> Vec<String> {
        let mut benchmarks = results
            .inner
            .keys()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        benchmarks.sort();
        benchmarks
    }

    /// Whether any benchmark holds a variant with these parameters.
    fn has_variant(results: &AdapterResults, parameters: &str) -> bool {
        let parameters: ParameterSet = parameters.parse().expect("Failed to parse parameters");
        results
            .inner
            .values()
            .any(|variants| variants.contains_key(&parameters))
    }

    /// The whole run's request count, if the results hold it.
    fn run_requests(results: &AdapterResults) -> Option<f64> {
        value(results, "gatling", "{}", Requests::SLUG_STR, "value")
    }

    /// Kills the OK and KO columns swapped or read from Total, `min` and `max` swapped, `mean`
    /// and `std_dev` swapped, a percentile row's time under another row's level, a time left in
    /// ms or scaled by another factor, the digits after a comma dropped, the rate read from
    /// another column, the counts read from another row or column, and the counts or a column
    /// written on the wrong variant.
    #[test]
    fn adapter_http_gatling_global_information() {
        let results = named("maven_spread.txt");
        assert_eq!(results.skipped, Skipped::default());
        assert_variant(
            &results,
            "gatling",
            "{}",
            &[
                latency([0.0, 3_002.0, 63.0, 308.0], [50.0, 51.0, 52.0, 2_787.0]),
                throughput(661.92),
                count(Requests::SLUG_STR, 8_605.0),
                count(FailedRequests::SLUG_STR, 347.0),
            ],
        );
        assert_variant(
            &results,
            "gatling",
            OK,
            &[
                latency([0.0, 3_002.0, 66.0, 314.0], [50.0, 51.0, 52.0, 2_867.0]),
                throughput(635.23),
            ],
        );
        assert_variant(
            &results,
            "gatling",
            KO,
            &[
                latency([0.0, 6.0, 0.0, 1.0], [0.0, 0.0, 1.0, 3.0]),
                throughput(26.69),
            ],
        );
    }

    /// Kills the run's own `Global` row read as a request, a table before the last read, a
    /// request's total and KO counts swapped or read from the OK column, and a request the
    /// scenario sends three times counted apart.
    #[test]
    fn adapter_http_gatling_requests_of_the_last_table() {
        let results = named("maven_spread.txt");
        assert_eq!(benchmarks(&results), ["fast", "gatling", "mixed", "slow"]);
        assert_variant(&results, "fast", "{}", &request(1_730.0, 0.0));
        assert_variant(&results, "slow", "{}", &request(5_172.0, 0.0));
        assert_variant(&results, "mixed", "{}", &request(1_703.0, 347.0));
        let results = named("mixed_3.16.0.txt");
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(
            benchmarks(&results),
            [
                "checkout / create",
                "checkout / error",
                "drop",
                "fast",
                "gatling",
                "mixed",
                "slow"
            ]
        );
        for (benchmark, total, ko) in [
            ("fast", 1_024.0, 0.0),
            ("checkout / create", 145_089.0, 0.0),
            ("checkout / error", 145_087.0, 145_087.0),
            ("drop", 145_086.0, 145_086.0),
            ("slow", 1_024.0, 0.0),
            ("mixed", 1_017.0, 258.0),
        ] {
            assert_variant(&results, benchmark, "{}", &request(total, ko));
        }
    }

    /// Kills a `-` cell refused or written as a time of 0, a variant made for a column with no
    /// request, and the failed requests left out when no request failed, whether Gatling prints
    /// that count as `-` (3.14.0 on) or as `0` (3.13.4).
    #[test]
    fn adapter_http_gatling_column_with_no_request() {
        let results = named("maven_drop.txt");
        assert_eq!(results.skipped, Skipped::default());
        assert_variant(
            &results,
            "gatling",
            "{}",
            &[
                latency([0.0, 5.0, 0.0, 0.0], [0.0, 0.0, 1.0, 1.0]),
                throughput(20_077.25),
                count(Requests::SLUG_STR, 80_309.0),
                count(FailedRequests::SLUG_STR, 80_309.0),
            ],
        );
        assert!(!has_variant(&results, OK), "an OK variant with no request");
        for (suffix, requests, max, p95, rate) in [
            ("fast_3.16.0.txt", 1_972_257.0, 8.0, 0.0, 179_296.09),
            ("fast_3.13.4.txt", 1_991_803.0, 9.0, 1.0, 181_073.0),
        ] {
            let results = named(suffix);
            assert_eq!(results.skipped, Skipped::default(), "{suffix}");
            assert_variant(
                &results,
                "gatling",
                "{}",
                &[
                    latency([0.0, max, 0.0, 0.0], [0.0, 0.0, p95, 1.0]),
                    throughput(rate),
                    count(Requests::SLUG_STR, requests),
                    count(FailedRequests::SLUG_STR, 0.0),
                ],
            );
            assert!(!has_variant(&results, KO), "a KO variant in {suffix}");
        }
    }

    /// Kills a level read from the label as another level, `99.9` as `p99` or `p999`, and a
    /// `100th` row written over the exact `max`. No capture's `100th` row differs from its
    /// `max`, so the test changes the `100th` row's Total in a real capture that prints one.
    #[test]
    fn adapter_http_gatling_percentile_levels() {
        let input = replaced(
            "maven_percentiles.txt",
            "> response time 100th percentile (ms)",
            "> response time 100th percentile (ms)                                                |        50 |        51 |         1",
        );
        let results = parsed(&input);
        // The run's two long names that Gatling truncates alike.
        assert_eq!(
            results.skipped,
            Skipped {
                variants: 1,
                ..Skipped::default()
            }
        );
        assert_variant(
            &results,
            "gatling",
            "{}",
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("min", 0.0),
                        ("max", 51e6),
                        ("mean", 0.0),
                        ("std_dev", 1e6),
                        ("p25", 0.0),
                        ("median", 0.0),
                        ("p99_9", 1e6),
                    ],
                ),
                throughput(46_357.5),
                count(Requests::SLUG_STR, 185_430.0),
                count(FailedRequests::SLUG_STR, 118.0),
            ],
        );
    }

    /// Kills a request the user names `Global` taken for the run's row, a name cut at its own
    /// ` | `, a name kept with its padding, a group path cut short, and the second of two names
    /// that Gatling truncates alike written over the first or dropped uncounted.
    #[test]
    fn adapter_http_gatling_request_names() {
        let results = named("maven_names.txt");
        assert_eq!(
            results.skipped,
            Skipped {
                variants: 1,
                ..Skipped::default()
            }
        );
        assert_eq!(
            benchmarks(&results),
            [
                "Global",
                "a request name long enough that the console table cuts it short, past eighty-tw...",
                "a | b | c",
                "café ☕",
                "closed",
                "gatling",
                "outer / inner / deep"
            ]
        );
        assert_variant(&results, "Global", "{}", &request(118.0, 0.0));
        assert_variant(&results, "a | b | c", "{}", &request(118.0, 118.0));
        assert_variant(&results, "closed", "{}", &request(190_220.0, 0.0));
        assert_eq!(run_requests(&results), Some(190_928.0));
    }

    /// Kills a run refused when its console is light and prints no request rows, or when it
    /// builds no reports and prints no Global Information, the requests of a run with no reports
    /// dropped, and the run of a run with no reports left out, read from a request's row or
    /// another column, or written with a statistic or a column variant.
    #[test]
    fn adapter_http_gatling_light_and_no_reports() {
        let results = named("maven_light.txt");
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(benchmarks(&results), ["gatling"]);
        assert_eq!(run_requests(&results), Some(186_257.0));
        let results = named("maven_no_reports.txt");
        assert_eq!(
            results.skipped,
            Skipped {
                variants: 1,
                ..Skipped::default()
            }
        );
        assert_eq!(benchmarks(&results).len(), 7);
        assert_variant(&results, "closed", "{}", &request(193_150.0, 0.0));
        assert_variant(&results, "gatling", "{}", &request(193_858.0, 118.0));
        assert!(!has_variant(&results, OK), "an OK variant with no reports");
        assert!(!has_variant(&results, KO), "a KO variant with no reports");
    }

    /// Kills a reader that needs Gatling's lines at the start of the input or refuses the lines
    /// Maven, Gradle, and sbt print around them, and a request table of 200 rows cut short.
    #[test]
    fn adapter_http_gatling_reads_each_build_tool() {
        for (suffix, requests) in [
            ("maven_mixed.txt", 450_483.0),
            ("maven_quiet_mixed.txt", 489_390.0),
            ("gradle_mixed.txt", 478_382.0),
            ("sbt_mixed.txt", 490_867.0),
            ("sbt2_mixed.txt", 497_136.0),
            ("sbt_fork_drop.txt", 73_231.0),
            ("mixed_3.13.4.txt", 455_703.0),
            ("maven_names_3.13.4.txt", 183_031.0),
        ] {
            let results = named(suffix);
            assert_eq!(results.skipped.benchmarks, 0, "{suffix}");
            assert_eq!(run_requests(&results), Some(requests), "{suffix}");
        }
        let results = named("large_3.16.0.txt");
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(benchmarks(&results).len(), 201);
        assert_variant(&results, "g / item 199", "{}", &request(100_804.0, 0.0));
    }

    /// Kills an input of several simulations read in part, whether the later simulation prints
    /// its own Global Information or only its tables, as a run with no reports or a run that
    /// fails does. The two edited inputs cut real captures of two simulations: one with the
    /// second simulation's tables taken out, one up to the second's Global Information.
    #[test]
    fn adapter_http_gatling_refuses_a_second_simulation() {
        for suffix in [
            "maven_multi.txt",
            "gradle_multi.txt",
            "sbt_multi.txt",
            "sbt2_multi.txt",
        ] {
            assert_eq!(parse(&fixture(suffix)), None, "{suffix}");
        }
        let multi = fixture("maven_multi.txt");
        let lines = multi.lines().collect::<Vec<_>>();
        let second = lines
            .iter()
            .position(|line| line.starts_with("[INFO] Running simulation bench.NamesSimulation"))
            .expect("Failed to find the second simulation");
        let second_global = lines
            .iter()
            .rposition(|line| line.starts_with("---- Global Information"))
            .expect("Failed to find the second Global Information");
        let without_tables = lines
            .iter()
            .take(second)
            .chain(lines.iter().skip(second_global - 1))
            .copied()
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(parse(&without_tables), None);
        let without_global = lines
            .iter()
            .take(second_global - 1)
            .copied()
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(parse(&without_global), None);
    }

    /// Kills an input of two simulations with reports off read in part, when the later
    /// simulation's first table is earlier in time than the table before it, and when it counts
    /// fewer requests, each read from a file and as `bencher run` passes it.
    #[test]
    fn adapter_http_gatling_refuses_a_later_simulation_with_no_reports() {
        for suffix in [
            "maven_multi_no_reports_time.txt",
            "maven_multi_no_reports_count.txt",
        ] {
            let raw = fixture(suffix);
            assert_eq!(parse(&raw), None, "{suffix}");
            assert_eq!(parse(&rebuilt(&raw)), None, "{suffix} through bencher run");
        }
    }

    /// Kills a table refused when the JVM's language names the time's zone with a space, read
    /// from a file and as `bencher run` passes it.
    #[test]
    fn adapter_http_gatling_zone_with_a_space() {
        let raw = fixture("maven_locale_am.txt");
        for input in [rebuilt(&raw), raw] {
            let results = parsed(&input);
            assert_eq!(results.skipped, Skipped::default());
            assert_eq!(benchmarks(&results), ["gatling", "home", "missing"]);
            assert_variant(&results, "home", "{}", &request(140.0, 0.0));
            assert_variant(&results, "missing", "{}", &request(140.0, 140.0));
            assert_eq!(run_requests(&results), Some(280.0));
        }
    }

    /// Kills a block cut short read in part or uncounted, a reader that goes on into the lines
    /// after the cut, and a table read whose rows do not sum to the run's. No capture holds a cut
    /// block, so the test cuts real captures: one inside its Global Information, joined to the
    /// lines Maven prints after it, one inside its last table, joined to the end of another run,
    /// and one with a row of its last table taken out, as a cut at a row's end leaves it.
    #[test]
    fn adapter_http_gatling_cut_block_reads_nothing_of_the_next() {
        let spread = fixture("maven_spread.txt");
        let lines = spread.lines().collect::<Vec<_>>();
        let global = lines
            .iter()
            .position(|line| line.starts_with("---- Global Information"))
            .expect("Failed to find the Global Information");
        let build = lines
            .iter()
            .position(|line| line.starts_with("[INFO] BUILD"))
            .expect("Failed to find the end of the build");
        let cut = lines
            .iter()
            .take(global + 5)
            .chain(lines.iter().skip(build))
            .copied()
            .collect::<Vec<_>>()
            .join("\n");
        let results = parsed(&cut);
        assert_eq!(
            results.skipped,
            Skipped {
                benchmarks: 1,
                ..Skipped::default()
            }
        );
        assert_eq!(benchmarks(&results), ["fast", "mixed", "slow"]);
        let table = lines
            .iter()
            .rposition(|line| line.starts_with("---- Requests"))
            .expect("Failed to find the last table");
        let drop = fixture("maven_drop.txt");
        let drop = drop.lines().collect::<Vec<_>>();
        let parsing = drop
            .iter()
            .position(|line| line.starts_with("Parsing log file(s)..."))
            .expect("Failed to find the reports");
        let cut = lines
            .iter()
            .take(table + 3)
            .chain(drop.iter().skip(parsing))
            .copied()
            .collect::<Vec<_>>()
            .join("\n");
        let results = parsed(&cut);
        assert_eq!(
            results.skipped,
            Skipped {
                benchmarks: 1,
                ..Skipped::default()
            }
        );
        assert_eq!(benchmarks(&results), ["gatling"]);
        assert_eq!(run_requests(&results), Some(80_309.0));
        let slow = lines
            .iter()
            .rposition(|line| line.starts_with("> slow "))
            .expect("Failed to find the last table's slow row");
        let cut = lines
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != slow)
            .map(|(_, line)| *line)
            .collect::<Vec<_>>()
            .join("\n");
        let results = parsed(&cut);
        assert_eq!(
            results.skipped,
            Skipped {
                benchmarks: 1,
                ..Skipped::default()
            }
        );
        assert_eq!(benchmarks(&results), ["gatling"]);
    }

    /// Kills a reader that passes over a line Gatling never prints where it reads: in the last
    /// table, a row with 2 counts, with a count not in groups of 3, with a decimal or a `-`
    /// count, whose total is not its OK and KO counts summed, with a leading zero, with no `> `,
    /// a first row that is not the run's, and a time line not Gatling's; in the Global
    /// Information, 3.13.3's labels, a time with a decimal, a label with no unit, a level with no
    /// ordinal, a request count whose total is not its columns summed, a rate with 3 decimals or
    /// a trailing zero, 3.13.3's range labels, a range count not in groups of 3, a line after the
    /// ranges that is neither the errors title nor the closing rule, and a table title one `-`
    /// short after the ranges. No capture holds such a line, so the test puts each in place of a
    /// line of a real capture, as an output cut short or two writers to one output leave it.
    #[test]
    fn adapter_http_gatling_refuses_a_foreign_row() {
        const PAD: &str =
            "                                                                               ";
        let table = |row: &str| (Table, "> slow ", format!("> slow{PAD}| {row}"));
        let global = |start: &'static str, line: String| (Run, start, line);
        for (part, start, line) in [
            table("    5,172 |     5,172"),
            table("     5172 |      5172 |         0"),
            table("  5,172.5 |   5,172.5 |         0"),
            table("        - |         - |         -"),
            table("    5,172 |     5,171 |         0"),
            table("   05,172 |     5,172 |         0"),
            (Table, "> slow ", "slow   |     5,172 |     5,172 |         0".to_owned()),
            (
                Table,
                "> Global ",
                format!("> Total{PAD}|     8,605 |     8,258 |       347"),
            ),
            (Table, "2026-", "2026-11-23 04:39:53 GMT".to_owned()),
            global(
                "> mean throughput (rps)",
                format!("> mean requests/sec{PAD}|    661.92 |    635.23 |     26.69"),
            ),
            global(
                "> min response time (ms)",
                format!("> min response time (ms){PAD}|       0.5 |         0 |         0"),
            ),
            global(
                "> min response time (ms)",
                format!("> min response time{PAD}|         0 |         0 |         0"),
            ),
            global(
                "> response time 95th percentile (ms)",
                format!("> response time 95 percentile (ms){PAD}|        52 |        52 |         1"),
            ),
            global(
                "> request count",
                format!("> request count{PAD}|     8,605 |     8,258 |       346"),
            ),
            global(
                "> mean throughput (rps)",
                format!("> mean throughput (rps){PAD}|    661.90 |    635.23 |     26.69"),
            ),
            global(
                "> mean throughput (rps)",
                format!("> mean throughput (rps){PAD}|   661.925 |    635.23 |     26.69"),
            ),
            global(
                "> OK: t < 800 ms",
                "> t < 800 ms                                                                                            8,165 (94.89%)"
                    .to_owned(),
            ),
            global(
                "> OK: t < 800 ms",
                "> OK: t < 800 ms                                                                                           8165 (94.89%)"
                    .to_owned(),
            ),
            global(
                "> OK: 800 ms <= t < 1200 ms",
                "> 800 ms <= t < 1200 ms                                                                                     0     (0%)"
                    .to_owned(),
            ),
            global(
                "> KO ",
                "> failed                                                                                                    347  (4.03%)"
                    .to_owned(),
            ),
            global("---- Errors", "Reports generated".to_owned()),
            global(
                "---- Errors",
                "---- Requests ----------------------------------------------------------------------|---Total---|-----OK----|----KO----"
                    .to_owned(),
            ),
        ] {
            let input = replaced("maven_spread.txt", start, &line);
            assert_refused(&input, part, &line);
        }
    }

    /// Kills a reader that passes over an opening rule not Gatling's before the Global
    /// Information, and a line after the last table's rows that is neither blank nor the errors
    /// title. No capture holds such a line, so the test puts each in place of a line of a real
    /// capture.
    #[test]
    fn adapter_http_gatling_refuses_a_foreign_rule_or_title() {
        let mut lines = fixture("maven_spread.txt")
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let global = lines
            .iter()
            .position(|line| line.starts_with("---- Global Information"))
            .expect("Failed to find the Global Information");
        if let Some(rule) = lines.get_mut(global - 1) {
            "=======".clone_into(rule);
        }
        assert_refused(&lines.join("\n"), Run, "the opening rule");
        let mut lines = fixture("maven_spread.txt")
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let table = lines
            .iter()
            .rposition(|line| line.starts_with("---- Requests"))
            .expect("Failed to find the last table");
        let errors = table
            + lines
                .iter()
                .skip(table)
                .position(|line| line.starts_with("---- Errors"))
                .expect("Failed to find the table's errors");
        if let Some(title) = lines.get_mut(errors) {
            "Parsing log file(s)...".clone_into(title);
        }
        assert_refused(&lines.join("\n"), Table, "the line after the rows");
    }

    /// The part of a run a foreign line is in.
    #[derive(Clone, Copy)]
    enum Part {
        Table,
        Run,
    }
    use Part::{Run, Table};

    fn assert_refused(input: &str, part: Part, case: &str) {
        let results = parsed(input);
        assert_eq!(results.skipped.benchmarks, 1, "{case}");
        match part {
            Table => {
                assert_eq!(benchmarks(&results), ["gatling"], "{case}");
            },
            Run => {
                assert_eq!(benchmarks(&results), ["fast", "mixed", "slow"], "{case}");
            },
        }
    }

    /// Kills the `∞` that Gatling prints for the rate of a run inside one millisecond refused,
    /// counted as skipped, or written. No capture's run is that short, so the test puts the
    /// cell in place of a real capture's Total rate.
    #[test]
    fn adapter_http_gatling_infinite_rate_is_skipped() {
        let input = replaced(
            "maven_spread.txt",
            "> mean throughput (rps)",
            "> mean throughput (rps)                                                              |         ∞ |    635.23 |     26.69",
        );
        let results = parsed(&input);
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(
            value(&results, "gatling", "{}", Throughput::SLUG_STR, "mean"),
            None
        );
        assert_eq!(
            value(&results, "gatling", OK, Throughput::SLUG_STR, "mean"),
            Some(635.23)
        );
        assert_eq!(run_requests(&results), Some(8_605.0));
    }

    /// Kills a capture read one way from a file and another as `bencher run` passes it, with no
    /// blank line at the start and no last line break.
    #[test]
    fn adapter_http_gatling_reads_bencher_run_output() {
        let mut paths = std::fs::read_dir("./tool_output/http/gatling")
            .expect("Failed to read the captures")
            .map(|entry| entry.expect("Failed to read a capture").path())
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let raw = std::fs::read_to_string(&path).expect("Failed to read a capture");
            assert_eq!(parse(&rebuilt(&raw)), parse(&raw), "{}", path.display());
        }
    }

    /// The input as `bencher run` passes it, with no blank line at the start and no last line
    /// break.
    fn rebuilt(raw: &str) -> String {
        raw.lines()
            .skip_while(|line| line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Kills a statistic picked by the run's average setting.
    #[test]
    fn adapter_http_gatling_ignores_average() {
        assert_ignores_average(LEAF, "http/gatling/maven_spread.txt");
    }
}
