use std::{collections::BTreeMap, iter::Peekable};

use bencher_json::{
    BenchmarkName, MeasureNameId, ParameterSet,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Latency, Memory, PeakMemory},
        },
        report::Adapter,
    },
};

use serde::Deserialize;

use crate::{
    Adaptable, Settings,
    adapters::{
        bmf_v1::{
            finite::{Finite, finite_json},
            leaf::{LeafResults, Variant},
        },
        util::Units,
    },
    results::adapter_results::AdapterResults,
};

/// airspeed velocity at version 1: a results file, then the text `asv run` prints.
pub struct AdapterPythonAsvV1;

impl Adaptable for AdapterPythonAsvV1 {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterPythonAsvJson::parse(input, settings)
            .or_else(|| AdapterPythonAsvText::parse(input, settings))
    }
}

/// The results file `asv run` writes for one commit and environment, read without the
/// `benchmarks.json` beside it, which alone names the parameters and holds the units.
///
/// A parameter is keyed by its position, `@param0` and on, and a `track_` result is skipped and
/// counted.
pub struct AdapterPythonAsvJson;

impl Adaptable for AdapterPythonAsvJson {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let ResultsFile {
            result_columns,
            results: rows,
        } = serde_json::from_str(&finite_json(input)).ok()?;
        let mut results = LeafResults::default();
        for (name, row) in rows {
            let column = |key: &str| {
                result_columns
                    .iter()
                    .position(|column| column == key)
                    .and_then(|index| row.get(index))
            };
            let read = |key: &str| {
                column(key)
                    .and_then(|values| Vec::<Finite>::deserialize(values).ok())
                    .unwrap_or_default()
            };
            let kind = Kind::of(&name).filter(|kind| !matches!(kind, Kind::Track));
            let values =
                column("result").and_then(|values| Vec::<Finite>::deserialize(values).ok());
            let params =
                column("params").and_then(|params| Vec::<Vec<String>>::deserialize(params).ok());
            let (Some(kind), Ok(benchmark_name), Some(values), Some(params)) =
                (kind, name.parse::<BenchmarkName>(), values, params)
            else {
                results.skip_benchmark();
                continue;
            };
            let stats = Stats {
                q_25: read("stats_q_25"),
                q_75: read("stats_q_75"),
                ci_99_a: read("stats_ci_99_a"),
                ci_99_b: read("stats_ci_99_b"),
            };
            for (index, (value, combination)) in
                values.into_iter().zip(combinations(&params)).enumerate()
            {
                let parameters = combination
                    .iter()
                    .enumerate()
                    .map(|(position, repr)| (format!("@param{position}"), parameter(shown(repr))))
                    .collect();
                let (Some(_), Ok(parameters)) = (
                    value.get(),
                    serde_json::from_value::<ParameterSet>(serde_json::Value::Object(parameters)),
                ) else {
                    results.skip_variant();
                    continue;
                };
                if let Some(mut variant) = results.new_variant(benchmark_name.clone(), parameters) {
                    kind.insert_result(&mut variant, value, &stats, index);
                }
            }
        }
        results.finish_with_skips(Adapter::PythonAsvJson)
    }
}

#[derive(Deserialize)]
struct ResultsFile {
    result_columns: Vec<String>,
    results: BTreeMap<String, Vec<serde_json::Value>>,
}

/// The statistics of a time benchmark's samples asv stores for each combination of its
/// parameters, where its interval at 99% is of the median.
struct Stats {
    q_25: Vec<Finite>,
    q_75: Vec<Finite>,
    ci_99_a: Vec<Finite>,
    ci_99_b: Vec<Finite>,
}

/// Every combination of the values of `params`, in the order asv stores their results.
fn combinations(params: &[Vec<String>]) -> Vec<Vec<&str>> {
    params
        .iter()
        .fold(vec![Vec::new()], |combinations, values| {
            combinations
                .iter()
                .flat_map(|combination| {
                    values.iter().map(move |value| {
                        let mut combination = combination.clone();
                        combination.push(value.as_str());
                        combination
                    })
                })
                .collect()
        })
}

/// A parameter's value as asv prints it from the representation its results file holds: a
/// string without its single quotes and a class by its name.
fn shown(repr: &str) -> &str {
    [("'", "'"), ("u'", "'"), ("<class '", "'>")]
        .into_iter()
        .find_map(|(start, end)| {
            repr.strip_prefix(start)?
                .strip_suffix(end)
                .filter(|inner| !inner.trim().is_empty())
        })
        .unwrap_or(repr)
}

/// The text `asv run` prints: a line per benchmark with its result, or with a table of its
/// results by parameter.
///
/// A time is asv's median, and its `±` is half the interquartile range. A `track_` result is
/// skipped and counted, since the text does not print its unit, and so is a table cell whose
/// column values cannot be told apart, where a value holds the ` / ` that joins them.
pub struct AdapterPythonAsvText;

impl Adaptable for AdapterPythonAsvText {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut results = LeafResults::default();
        let mut lines = input.lines().peekable();
        while let Some(line) = lines.next() {
            let Some((name, info)) = progress(line).and_then(|text| text.split_once(' ')) else {
                continue;
            };
            let Some(kind) = Kind::of(name) else {
                continue;
            };
            let Ok(benchmark_name) = name.parse::<BenchmarkName>() else {
                results.skip_benchmark();
                continue;
            };
            if let Some((indent, spans)) = lines.peek().and_then(|line| separator(line)) {
                lines.next();
                if let Some(table) = Table::read(&mut lines, indent, &spans) {
                    table.insert(&mut results, &benchmark_name, kind);
                } else {
                    results.skip_benchmark();
                }
            } else if let Some(values) = kind.values(info.trim()) {
                if let Some(mut variant) =
                    results.new_variant(benchmark_name, ParameterSet::default())
                {
                    insert(&mut variant, values);
                }
            } else {
                results.skip_benchmark();
            }
        }
        results.finish_with_skips(Adapter::PythonAsvText)
    }
}

/// The text after a line's progress mark and the three dots of a benchmark's line.
fn progress(line: &str) -> Option<&str> {
    let (percent, text) = line.strip_prefix('[')?.split_once("%] ")?;
    percent.trim().parse::<f64>().ok()?;
    text.strip_prefix("\u{b7}\u{b7}\u{b7} ")
}

/// What a benchmark measures, from the name asv gives each kind of benchmark.
#[derive(Clone, Copy)]
enum Kind {
    Time,
    Memory,
    PeakMemory,
    Track,
}

impl Kind {
    /// The kind asv gives a benchmark, from the last part of its name.
    fn of(name: &str) -> Option<Self> {
        let attribute = name.rsplit('.').next().unwrap_or(name);
        [
            ("time", "Time", Self::Time),
            ("timeraw", "Timeraw", Self::Time),
            ("mem", "Mem", Self::Memory),
            ("peakmem", "PeakMem", Self::PeakMemory),
            ("track", "Track", Self::Track),
        ]
        .into_iter()
        .find(|(prefix, class, _)| {
            attribute
                .strip_prefix(prefix)
                .and_then(|rest| rest.strip_prefix('_'))
                .is_some_and(|rest| !rest.is_empty())
                || attribute.strip_prefix(class).is_some_and(|rest| {
                    rest.chars()
                        .next()
                        .is_some_and(|next| next.is_ascii_uppercase() || next == '_')
                        && rest.chars().count() > 1
                })
        })
        .map(|(_, _, kind)| kind)
    }

    /// The metrics of the result at `index` of a results file: a time in seconds with its
    /// statistics, or a size in bytes.
    fn insert_result(self, variant: &mut Variant<'_>, value: Finite, stats: &Stats, index: usize) {
        let at = |column: &[Finite]| column.get(index).copied().unwrap_or_default();
        match self {
            Self::Time => {
                let latency = Latency::name_id();
                let factor = Units::Sec.as_nanos();
                value.insert(variant, &latency, "median", factor);
                at(&stats.q_25).insert(variant, &latency, "p25", factor);
                at(&stats.q_75).insert(variant, &latency, "p75", factor);
                at(&stats.ci_99_a).insert_spread(
                    value,
                    variant,
                    &latency,
                    "median_ci99_lower",
                    factor,
                );
                at(&stats.ci_99_b).insert_spread(
                    value,
                    variant,
                    &latency,
                    "median_ci99_upper",
                    factor,
                );
            },
            Self::Memory => value.insert(variant, &Memory::name_id(), "value", 1.0),
            Self::PeakMemory => value.insert(variant, &PeakMemory::name_id(), "value", 1.0),
            Self::Track => {},
        }
    }

    /// The metrics of a result as asv prints it, or `None` for one it does not hold a number.
    fn values(self, text: &str) -> Option<Vec<(MeasureNameId, &'static str, f64)>> {
        match self {
            Self::Time => {
                let (number, factor) = TIME_UNITS
                    .iter()
                    .find_map(|(unit, factor)| Some((text.strip_suffix(unit)?, *factor)))?;
                let (median, spread) = match number.split_once('\u{b1}') {
                    Some((median, spread)) => (median, Some(spread)),
                    None => (number, None),
                };
                let mut values = vec![(Latency::name_id(), "median", finite(median)? * factor)];
                if let Some(spread) = spread {
                    values.push((Latency::name_id(), "iqr", 2.0 * finite(spread)? * factor));
                }
                Some(values)
            },
            Self::Memory | Self::PeakMemory => {
                let (number, factor) = SIZE_SUFFIXES
                    .iter()
                    .find_map(|(suffix, factor)| Some((text.strip_suffix(suffix)?, *factor)))
                    .unwrap_or((text, 1.0));
                let measure = if let Self::Memory = self {
                    Memory::name_id()
                } else {
                    PeakMemory::name_id()
                };
                Some(vec![(measure, "value", finite(number)? * factor)])
            },
            Self::Track => None,
        }
    }
}

/// The units asv prints a time in, longest first where one ends another, and each in
/// nanoseconds.
const TIME_UNITS: [(&str, f64); 9] = [
    ("ns", 1.0),
    ("\u{3bc}s", 1e3),
    ("\u{b5}s", 1e3),
    ("ms", 1e6),
    ("s", 1e9),
    ("m", 60e9),
    ("h", 3_600e9),
    ("d", 86_400e9),
    ("w", 604_800e9),
];

/// The suffixes asv prints a size in bytes with.
const SIZE_SUFFIXES: [(&str, f64); 7] = [
    ("k", 1e3),
    ("M", 1e6),
    ("G", 1e9),
    ("T", 1e12),
    ("P", 1e15),
    ("E", 1e18),
    ("H", 1e21),
];

fn finite(number: &str) -> Option<f64> {
    number
        .parse()
        .ok()
        .filter(|number: &f64| number.is_finite())
}

fn insert(variant: &mut Variant<'_>, values: Vec<(MeasureNameId, &str, f64)>) {
    for (measure, metric, value) in values {
        variant.insert(measure, metric, value);
    }
}

/// The table asv prints under a parametrized benchmark's line: the names of the parameters of
/// its rows, then of its columns, the values of the column parameters of each result column, or
/// `None` where a value holds the ` / ` that joins them, and each row's values, then results.
struct Table {
    names: Vec<String>,
    row_parameters: usize,
    columns: Vec<Option<Vec<String>>>,
    rows: Vec<Vec<String>>,
}

impl Table {
    /// The lines of the table whose first rule, at `indent` characters with columns `spans`,
    /// was the line before, or `None` when they do not make one.
    fn read<'a, I>(lines: &mut Peekable<I>, indent: usize, spans: &[(usize, usize)]) -> Option<Self>
    where
        I: Iterator<Item = &'a str>,
    {
        let mut next = || {
            let line = body(lines.peek()?, indent)?;
            lines.next();
            Some(line)
        };
        // The header ends at the `=` rule under a top header, else at its `-` rule.
        let mut header = Vec::new();
        loop {
            let line = next()?;
            if is_rule(line, '=') {
                break;
            }
            header.push(line);
            if is_rule(line, '-') && header.first().is_some_and(|top| !top.starts_with("--")) {
                break;
            }
        }
        let mut rows = Vec::new();
        loop {
            let line = next()?;
            if is_rule(line, '=') {
                break;
            }
            rows.push(cells(line, spans));
        }
        let (names, row_parameters, columns) = match header.as_slice() {
            [top, rule, header] if is_rule(rule, '-') => {
                let space = rule.find(' ')?;
                let row_parameters = spans.iter().take_while(|(_, end)| *end <= space).count();
                let mut names = cells(header, spans);
                let values = names.split_off(row_parameters);
                let column_names = top
                    .strip_prefix("--")?
                    .trim()
                    .split(" / ")
                    .collect::<Vec<_>>();
                let columns = values
                    .iter()
                    .map(|values| {
                        let values = if column_names.len() == 1 {
                            vec![values.clone()]
                        } else {
                            values.split(" / ").map(ToOwned::to_owned).collect()
                        };
                        (values.len() == column_names.len()).then_some(values)
                    })
                    .collect();
                names.extend(column_names.into_iter().map(ToOwned::to_owned));
                (names, row_parameters, columns)
            },
            [header, rule] if is_rule(rule, '-') => {
                let mut names = cells(header, spans);
                names.pop();
                let row_parameters = names.len();
                (names, row_parameters, vec![Some(Vec::new())])
            },
            _ => return None,
        };
        Some(Self {
            names,
            row_parameters,
            columns,
            rows,
        })
    }

    /// Each result as the variant its row's and column's values make.
    fn insert(self, results: &mut LeafResults, benchmark_name: &BenchmarkName, kind: Kind) {
        let Self {
            names,
            row_parameters,
            columns,
            mut rows,
        } = self;
        for row in &mut rows {
            let cells = row.split_off(row_parameters);
            for (cell, column) in cells.iter().zip(&columns) {
                let Some(column) = column else {
                    results.skip_variant();
                    continue;
                };
                let parameters = names
                    .iter()
                    .zip(row.iter().chain(column))
                    .map(|(name, value)| (name.clone(), parameter(value)))
                    .collect::<serde_json::Map<_, _>>();
                let parameters = (parameters.len() == names.len())
                    .then(|| serde_json::from_value(serde_json::Value::Object(parameters)).ok())
                    .flatten();
                let (Some(parameters), Some(values)) = (parameters, kind.values(cell)) else {
                    results.skip_variant();
                    continue;
                };
                if let Some(mut variant) = results.new_variant(benchmark_name.clone(), parameters) {
                    insert(&mut variant, values);
                }
            }
        }
    }
}

/// A parameter's value as asv prints it: a number, or else the text, which is a string's
/// value without its quotes or the representation of any other value.
fn parameter(value: &str) -> serde_json::Value {
    finite(value).map_or_else(|| value.into(), Into::into)
}

/// The indent and the columns of a table's first rule, printed on its benchmark's progress line.
fn separator(line: &str) -> Option<(usize, Vec<(usize, usize)>)> {
    let rule = progress(line)?;
    if !is_rule(rule, '=') {
        return None;
    }
    let mut spans = Vec::new();
    let mut start = None;
    for (index, character) in rule.trim_end().chars().chain([' ']).enumerate() {
        match (character, start) {
            ('=', None) => start = Some(index),
            (' ', Some(begin)) => {
                spans.push((begin, index));
                start = None;
            },
            _ => {},
        }
    }
    Some((line.chars().count() - rule.chars().count(), spans))
}

/// A line of `=` or `-` runs.
fn is_rule(line: &str, character: char) -> bool {
    let line = line.trim_end();
    line.starts_with(character) && line.chars().all(|c| c == character || c == ' ')
}

/// The line past `indent` spaces, or `None` when it does not have them.
fn body(line: &str, indent: usize) -> Option<&str> {
    let (index, _) = line.char_indices().nth(indent)?;
    line.get(..index)?
        .chars()
        .all(|c| c == ' ')
        .then(|| line.get(index..))?
}

/// The text of each column, without the padding that centers it.
fn cells(line: &str, spans: &[(usize, usize)]) -> Vec<String> {
    let characters = line.chars().collect::<Vec<_>>();
    spans
        .iter()
        .map(|(start, end)| {
            characters
                .get(*start..(*end).min(characters.len()))
                .unwrap_or_default()
                .iter()
                .collect::<String>()
                .trim()
                .to_owned()
        })
        .collect()
}

#[cfg(test)]
mod test_python_asv_v1 {
    use bencher_json::project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Latency, Memory, PeakMemory},
        },
        report::Adapter,
    };
    use std::collections::BTreeSet;

    use pretty_assertions::assert_eq;

    use crate::{
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_value, assert_variant, convert_v1, variants,
        },
        results::adapter_results::AdapterResults,
    };

    const RESULTS: &str = "python/asv/results.json";
    const SHAPES: &str = "python/asv/shapes.json";
    const SAMPLES: &str = "python/asv/samples.json";
    const PARAMS: &str = "python/asv/params.txt";
    const COMMITS: &str = "python/asv/commits.txt";
    const SIX: &str = "python/asv/six.txt";
    const EMPTY_JSON: &str = "python/asv/empty.json";
    const EMPTY_TEXT: &str = "python/asv/empty.txt";

    fn latency(median: f64, iqr: f64) -> Expected {
        (Latency::SLUG_STR, vec![("median", median), ("iqr", iqr)])
    }

    /// asv's median, quartiles, and 99% interval of the median, in nanoseconds.
    fn time(median: f64, [p25, p75, lower, upper]: [f64; 4]) -> Expected {
        (
            Latency::SLUG_STR,
            vec![
                ("median", median),
                ("p25", p25),
                ("p75", p75),
                ("median_ci99_lower", lower),
                ("median_ci99_upper", upper),
            ],
        )
    }

    /// Kills a combination of `params` read in any order but asv's, with the last parameter
    /// varying fastest, a parameter keyed by anything but its position, a string's quotes kept, a
    /// number left a string, the result read as anything but the median, the quartiles or the
    /// interval's bounds swapped, misnamed, or read from the wrong column, a time left in
    /// seconds, a failed combination or a failed benchmark stored or left uncounted, a `track_`
    /// benchmark, whose unit only `benchmarks.json` holds, stored or left uncounted, memory read
    /// as a time, and a node or leaf that records no leaf.
    #[test]
    fn adapter_python_asv_json_v1_results() {
        for adapter in [Adapter::PythonAsv, Adapter::PythonAsvJson] {
            let results = convert_v1(adapter, RESULTS);
            assert_eq!(results.resolved_adapter, Some(Adapter::PythonAsvJson));
            for (parameters, expected) in [
                (
                    r#"{"@param0":10,"@param1":"a"}"#,
                    time(150.313_706_873_016_5, [145.74, 153.87, 141.86, 158.29]),
                ),
                (
                    r#"{"@param0":10,"@param1":"b c"}"#,
                    time(146.014_166_303_222_25, [145.92, 149.55, 143.37, 152.09]),
                ),
                (
                    r#"{"@param0":100,"@param1":"a"}"#,
                    time(420.410_815_758_221_1, [413.39, 430.21, 406.2, 437.84]),
                ),
                (
                    r#"{"@param0":100,"@param1":"b c"}"#,
                    time(427.257_233_692_439_06, [393.89, 475.27, 361.07, 508.6]),
                ),
            ] {
                assert_close(
                    &results,
                    "benchmarks.Params.time_join",
                    parameters,
                    &expected,
                );
            }
            for (parameters, expected) in [
                (
                    r#"{"@param0":1,"@param1":0.5,"@param2":"y / z"}"#,
                    time(124.723_448_013_854_24, [114.98, 141.68, 104.61, 151.28]),
                ),
                (
                    r#"{"@param0":2,"@param1":1e-05,"@param2":"y / z"}"#,
                    time(151.827_903_342_496_54, [140.05, 163.64, 131.07, 172.73]),
                ),
            ] {
                assert_close(&results, "benchmarks.Wide.time_wide", parameters, &expected);
            }
            assert_close(
                &results,
                "benchmarks.time_single_param",
                r#"{"@param0":1000}"#,
                &time(5_591.692_328_274_674, [5_584.0, 5_599.2, 5_562.0, 5_609.2]),
            );
            assert_close(
                &results,
                "benchmarks.time_slow",
                "{}",
                &time(
                    2_056_006.399_925_536_2,
                    [2_054_500.0, 2_058_500.0, 2_052_500.0, 2_060_400.0],
                ),
            );
            assert_variant(
                &results,
                "benchmarks.MemSuite.mem_list",
                "{}",
                &[(Memory::SLUG_STR, vec![("value", 2_104.0)])],
            );
            assert_variant(
                &results,
                "benchmarks.peakmem_list",
                "{}",
                &[(PeakMemory::SLUG_STR, vec![("value", 27_172_864.0)])],
            );
            assert_eq!(variants(&results), 17);
            assert_eq!(results.skipped.benchmarks, 2);
            assert_eq!(results.skipped.variants, 2);
            assert_eq!(results.skipped.metrics, 0);
        }
    }

    /// Kills a value's representation read in any other way than asv prints it (a string in
    /// double quotes kept whole, a class by its name), a combination asv skipped (`NaN`) or a
    /// failed one stored or left uncounted, a ninth parameter kept, the infinite interval asv
    /// writes for a single sample stored or counted, a `timeraw_` benchmark or a parametrized
    /// memory benchmark dropped, and a failed or a `track_` benchmark left uncounted.
    #[test]
    fn adapter_python_asv_json_v1_shapes() {
        let results = convert_v1(Adapter::PythonAsvJson, SHAPES);
        for (parameters, expected) in [
            (
                r#"{"@param0":1,"@param1":"\"'quoted'\""}"#,
                time(230.000_068_768_276_83, [223.0, 231.0, 220.0, 240.0]),
            ),
            (
                r#"{"@param0":2,"@param1":"\"'quoted'\""}"#,
                time(250.999_960_371_700_58, [250.25, 260.0, 240.0, 261.0]),
            ),
        ] {
            assert_close(&results, "shapes.Grid.time_grid", parameters, &expected);
        }
        for (value, median) in [
            (r#""None""#, 46.279_817_114_860_315),
            ("1.5", 45.538_733_246_525_254),
            (r#""s""#, 44.120_826_486_784_91),
            (r#""(1, 2)""#, 44.182_452_300_737_3),
            (r#""int""#, 44.955_155_230_945_82),
        ] {
            assert_value(
                &results,
                ("shapes.time_values", &format!(r#"{{"@param0":{value}}}"#)),
                (Latency::SLUG_STR, "median"),
                median,
            );
        }
        assert_close(
            &results,
            "shapes.time_once",
            "{}",
            &(
                Latency::SLUG_STR,
                vec![
                    ("median", 631.000_148_132_443_4),
                    ("p25", 631.0),
                    ("p75", 631.0),
                ],
            ),
        );
        assert_close(
            &results,
            "shapes.timeraw_import",
            "{}",
            &time(
                3_476_008.000_006_913_7,
                [3_432_700.0, 3_530_400.0, 3_408_100.0, 3_723_200.0],
            ),
        );
        assert_close(
            &results,
            "shapes.time_partial",
            r#"{"@param0":1}"#,
            &time(88.381_239_987_855_66, [87.555, 89.128, 87.37, 89.879]),
        );
        for (n, label, memory) in [
            (1, "\"'quoted'\"", 568.0),
            (1, "plain", 568.0),
            (2, "\"'quoted'\"", 1_080.0),
        ] {
            let parameters = serde_json::json!({ "@param0": n, "@param1": label }).to_string();
            assert_variant(
                &results,
                "shapes.Grid.mem_grid",
                &parameters,
                &[(Memory::SLUG_STR, vec![("value", memory)])],
            );
            assert_variant(
                &results,
                "shapes.Grid.peakmem_grid",
                &parameters,
                &[(PeakMemory::SLUG_STR, vec![("value", 24_539_136.0)])],
            );
        }
        assert_eq!(variants(&results), 17);
        assert_eq!(results.skipped.benchmarks, 2);
        assert_eq!(results.skipped.variants, 5);
        assert_eq!(results.skipped.metrics, 0);
    }

    /// Kills any metric derived from the `samples` column `--record-samples` adds, which the leaf
    /// leaves unread, and that column misread for any other.
    #[test]
    fn adapter_python_asv_json_v1_samples() {
        let results = convert_v1(Adapter::PythonAsvJson, SAMPLES);
        for (parameters, expected) in [
            (
                r#"{"@param0":1,"@param1":"a"}"#,
                time(73.016_302_445_761_39, [72.933, 73.054, 72.702, 74.08]),
            ),
            (
                r#"{"@param0":2,"@param1":"a"}"#,
                time(75.642_399_202_117_24, [73.325, 78.121, 73.198, 78.293]),
            ),
        ] {
            assert_close(
                &results,
                "benchmarks.Params.time_join",
                parameters,
                &expected,
            );
        }
        assert_close(
            &results,
            "benchmarks.time_sleep",
            "{}",
            &time(
                552_752.921_054_745_6,
                [551_570.0, 553_270.0, 550_950.0, 557_410.0],
            ),
        );
        assert_close(
            &results,
            "benchmarks.mem_list",
            "{}",
            &(Memory::SLUG_STR, vec![("value", 568.0)]),
        );
        assert_eq!(variants(&results), 4);
        assert_eq!(results.skipped.benchmarks, 2);
        assert_eq!(results.skipped.variants, 0);
        assert_eq!(results.skipped.metrics, 0);
    }

    /// Kills the empty string asv writes as `''` stored without its quotes, as `""`, where asv
    /// prints it as `''`, and a string holding a single quote stored without its double quotes.
    #[test]
    fn adapter_python_asv_json_v1_empty() {
        let results = convert_v1(Adapter::PythonAsvJson, EMPTY_JSON);
        for (size, kind, median) in [
            (10, "a", 212.502_572_578_676_75),
            (10, "''", 193.665_318_412_421_8),
            (10, "\"it's\"", 151.779_839_123_142_92),
            (100, "a", 532.001_819_932_767),
            (100, "''", 390.690_869_436_723_1),
            (100, "\"it's\"", 524.015_907_469_867_4),
        ] {
            let parameters = serde_json::json!({ "@param0": size, "@param1": kind }).to_string();
            assert_value(
                &results,
                ("benchmarks.Params.time_join", &parameters),
                (Latency::SLUG_STR, "median"),
                median,
            );
        }
        assert_eq!(variants(&results), 6);
        assert!(results.skipped.is_empty());
    }

    /// Kills a function asv times for its CamelCase name, `Time_x`, read as no kind and skipped,
    /// as a leaf does that refuses a `_` after the kind's name or a tail of more than one
    /// character, beside a class's `time_x`.
    #[test]
    fn adapter_python_asv_json_v1_kinds() {
        let results = convert_v1(Adapter::PythonAsvJson, "python/asv/kinds.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_eq!(variants(&results), 2);
        for (benchmark, expected) in [
            (
                "benchmarks.TimeSuite.time_x",
                time(438.706_314_132_160_7, [420.69, 459.28, 385.75, 523.59]),
            ),
            (
                "benchmarks.Time_x",
                time(392.764_420_769_274_75, [387.83, 409.55, 358.84, 428.31]),
            ),
        ] {
            assert_variant(&results, benchmark, "{}", &[expected]);
        }
    }

    /// Kills the node without its results leaf, or with either leaf missing.
    #[test]
    fn adapter_python_asv_v1_node() {
        for (suffix, leaf) in [
            (RESULTS, Adapter::PythonAsvJson),
            (SHAPES, Adapter::PythonAsvJson),
            (PARAMS, Adapter::PythonAsvText),
            (SIX, Adapter::PythonAsvText),
        ] {
            let results = convert_v1(Adapter::PythonAsv, suffix);
            assert_eq!(results.resolved_adapter, Some(leaf), "{suffix}");
            assert_eq!(results, convert_v1(leaf, suffix), "{suffix}");
        }
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_python_asv_json_v1_ignores_average() {
        for suffix in [RESULTS, SHAPES, SAMPLES, EMPTY_JSON] {
            assert_ignores_average(Adapter::PythonAsvJson, suffix);
        }
    }

    /// The variant's measure holds the expected metrics to within floating point rounding, and no
    /// others.
    fn assert_close(
        results: &AdapterResults,
        benchmark: &str,
        parameters: &str,
        (measure, expected): &Expected,
    ) {
        let actual = results.inner[&benchmark.parse().expect("Failed to parse benchmark")]
            [&parameters.parse().expect("Failed to parse parameters")]
            .inner[&measure.parse().expect("Failed to parse measure")]
            .inner
            .keys()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual,
            expected
                .iter()
                .map(|(name, _)| (*name).to_owned())
                .collect(),
            "{benchmark} {parameters}"
        );
        for (metric, value) in expected {
            assert_value(results, (benchmark, parameters), (measure, metric), *value);
        }
    }

    /// Kills the parameter tables dropped, as version 0 drops them, a cell read from the wrong
    /// column, a row parameter and a column parameter swapped, a number left a string, the
    /// spread stored as printed rather than as the interquartile range it is half of, a time
    /// left in its printed unit, and a node or leaf that records no leaf.
    #[test]
    fn adapter_python_asv_text_v1_tables() {
        for adapter in [Adapter::PythonAsv, Adapter::PythonAsvText] {
            let results = convert_v1(adapter, PARAMS);
            assert_eq!(results.resolved_adapter, Some(Adapter::PythonAsvText));
            for (size, kind, expected) in [
                (10, "a", latency(150.0, 8.0)),
                (10, "b c", latency(146.0, 4.0)),
                (100, "a", latency(420.0, 16.0)),
                (100, "b c", latency(427.0, 80.0)),
            ] {
                assert_variant(
                    &results,
                    "benchmarks.Params.time_join",
                    &format!(r#"{{"size":{size},"kind":"{kind}"}}"#),
                    &[expected],
                );
            }
            for (param, expected) in [(1, latency(77.5, 12.0)), (1000, latency(5_360.0, 400.0))] {
                assert_variant(
                    &results,
                    "benchmarks.time_single_param",
                    &format!(r#"{{"param1":{param}}}"#),
                    &[expected],
                );
            }
        }
    }

    /// Kills a failed cell stored or left uncounted, a column header split at a ` / ` a value
    /// holds (`0.5 / y / z` is `0.5` and `y / z` or `0.5 / y` and `z`) read as either, a failed
    /// benchmark or a `track_` benchmark, whose unit the text does not print, stored or left
    /// uncounted, memory read as a time, and `ms` read as minutes.
    #[test]
    fn adapter_python_asv_text_v1_shapes() {
        let results = convert_v1(Adapter::PythonAsvText, PARAMS);
        for (scale, expected) in [
            ("0.5", latency(124.0, 18.0)),
            ("1e-05", latency(124.0, 20.0)),
        ] {
            assert_variant(
                &results,
                "benchmarks.Wide.time_wide",
                &format!(r#"{{"n":1,"scale":{scale},"label":"x"}}"#),
                &[expected],
            );
        }
        for (name, expected) in [
            (
                "benchmarks.TimeSuite.time_iterkeys",
                latency(2_120.0, 600.0),
            ),
            ("benchmarks.TimeSuite.time_keys", latency(2_100.0, 600.0)),
            ("benchmarks.time_slow", latency(2_060_000.0, 0.0)),
            (
                "benchmarks.MemSuite.mem_list",
                (Memory::SLUG_STR, vec![("value", 2_100.0)]),
            ),
            (
                "benchmarks.peakmem_list",
                (PeakMemory::SLUG_STR, vec![("value", 27_200_000.0)]),
            ),
        ] {
            assert_variant(&results, name, "{}", &[expected]);
        }
        assert_eq!(variants(&results), 13);
        assert_eq!(results.skipped.benchmarks, 2);
        assert_eq!(results.skipped.variants, 6);
    }

    /// Kills a benchmark a later commit's run repeats stored over the first, or left uncounted,
    /// and a table indented one column further, under a wider progress mark, left unread.
    #[test]
    fn adapter_python_asv_text_v1_repeats() {
        let results = convert_v1(Adapter::PythonAsvText, COMMITS);
        assert_variant(
            &results,
            "benchmarks.TimeSuite.time_iterkeys",
            "{}",
            &[latency(2_090.0, 160.0)],
        );
        assert_variant(
            &results,
            "benchmarks.TimeSuite.time_keys",
            "{}",
            &[latency(2_150.0, 40.0)],
        );
        assert_variant(
            &results,
            "benchmarks.time_single_param",
            r#"{"param1":1000}"#,
            &[latency(5_590.0, 20.0)],
        );
        assert_eq!(variants(&results), 4);
        assert_eq!(results.skipped.variants, 4);
        assert_eq!(results.skipped.benchmarks, 0);
    }

    /// Kills memory dropped, as version 0 drops it, and a failed benchmark left uncounted, in an
    /// older asv's output over two environments.
    #[test]
    fn adapter_python_asv_text_v1_six() {
        let results = convert_v1(Adapter::PythonAsvText, SIX);
        assert_eq!(variants(&results), 8);
        assert_eq!(results.skipped.benchmarks, 2);
        assert_variant(
            &results,
            "benchmarks.MemSuite.mem_list",
            "{}",
            &[(Memory::SLUG_STR, vec![("value", 2_420.0)])],
        );
        assert_variant(
            &results,
            "benchmarks.TimeSuite.time_keys3",
            "{}",
            &[latency(9_070.0, 1_000.0)],
        );
    }

    /// Kills a string's quotes stripped from a cell, which reads asv's `''` as the empty string and
    /// `"it's"` without its double quotes, where the results file holds them as printed.
    #[test]
    fn adapter_python_asv_text_v1_empty() {
        let results = convert_v1(Adapter::PythonAsvText, EMPTY_TEXT);
        for (size, kind, expected) in [
            (10, "a", latency(213.0, 120.0)),
            (10, "''", latency(194.0, 100.0)),
            (10, "\"it's\"", latency(152.0, 10.0)),
            (100, "a", latency(532.0, 80.0)),
            (100, "''", latency(391.0, 40.0)),
            (100, "\"it's\"", latency(524.0, 8.0)),
        ] {
            let parameters = serde_json::json!({ "size": size, "kind": kind }).to_string();
            assert_variant(
                &results,
                "benchmarks.Params.time_join",
                &parameters,
                &[expected],
            );
        }
        assert_eq!(variants(&results), 6);
        assert!(results.skipped.is_empty());
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_python_asv_text_v1_ignores_average() {
        for suffix in [PARAMS, COMMITS, SIX, EMPTY_TEXT] {
            assert_ignores_average(Adapter::PythonAsvText, suffix);
        }
    }
}
