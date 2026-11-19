use bencher_json::{
    MeasureNameId,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Cycles, Latency},
            iai::Instructions,
            mitata::Branches,
        },
        report::Adapter,
    },
};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        cpp::nanobench::{insert_rate, variant_with_context},
        finite::Finite,
        leaf::LeafResults,
    },
    results::adapter_results::AdapterResults,
};

/// nanobench's default markdown table: a header that ends in the `Bench`'s title over a rule of
/// right-aligned columns, then a row per result, each time and counter per unit.
pub struct AdapterCppNanobenchText;

impl Adaptable for AdapterCppNanobenchText {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut results = LeafResults::default();
        let mut claimed = false;
        // A table stays open until the next header, since a benchmark's own print can fall
        // between two rows.
        let mut table = Open::None;
        let mut lines = input.lines().peekable();
        while let Some(line) = lines.next() {
            if let Some(count) = lines.peek().and_then(|rule| columns(rule))
                && line.starts_with('|')
            {
                table = Table::new(line, count).map_or(Open::Unread, Open::Table);
                claimed |= matches!(table, Open::Table(_));
                lines.next();
                continue;
            }
            if !line.starts_with('|') {
                continue;
            }
            match &table {
                Open::Table(table) => table.insert(&mut results, line),
                Open::Unread => results.skip_benchmark(),
                Open::None => {},
            }
        }
        claimed
            .then(|| results.finish_with_skips(Adapter::CppNanobenchText))
            .flatten()
    }
}

/// The table the rows that follow belong to.
enum Open {
    None,
    Table(Table),
    /// A header the leaf cannot read, such as one with its time or rate column hidden.
    Unread,
}

/// One header's columns, each with the width nanobench prints it at, with the `Bench`'s title
/// and unit.
struct Table {
    columns: Vec<(Column, usize)>,
    title: String,
    unit: String,
}

impl Table {
    /// The table that `header` opens over a rule of `count` columns, if the leaf can read it: a
    /// time per unit followed by its rate, and no column nanobench does not print.
    fn new(header: &str, count: usize) -> Option<Self> {
        let mut cells = header.strip_prefix('|')?.split('|');
        let titles = cells
            .by_ref()
            .take(count)
            .map(str::trim)
            .collect::<Vec<_>>();
        let title = cells.collect::<Vec<_>>().join("|");
        let title = title.strip_prefix(' ')?.to_owned();
        let (rate, time, unit) = titles
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(index, rate)| {
                let unit = rate.strip_suffix("/s")?;
                let time = titles
                    .get(index - 1)?
                    .strip_suffix(unit)?
                    .strip_suffix('/')?;
                Some((index, time, unit))
            })?;
        let columns = titles
            .iter()
            .enumerate()
            .map(|(index, column)| {
                if index == rate {
                    Some((Column::Rate, 22))
                } else if index + 1 == rate {
                    Some((Column::Time(nanoseconds(time)), 22))
                } else if index + 1 < rate {
                    Some(Column::leading(column))
                } else {
                    Column::trailing(column, unit)
                }
            })
            .collect::<Option<_>>()?;
        Some(Self {
            columns,
            title,
            unit: unit.to_owned(),
        })
    }

    /// The result of `row`, or a skip counted when the row is an error or cannot be read.
    fn insert(&self, results: &mut LeafResults, row: &str) {
        let Some((cells, name)) = split(row) else {
            results.skip_benchmark();
            return;
        };
        let Some(columns) = self.columns(&cells) else {
            results.skip_benchmark();
            return;
        };
        let complexity_n = columns
            .iter()
            .zip(&cells)
            .find(|((column, _), _)| matches!(column, Column::ComplexityN))
            .and_then(|(_, cell)| number(cell));
        // nanobench leaves the cell blank when the row does not set the variable.
        let context = columns
            .iter()
            .zip(&cells)
            .filter_map(|((column, _), cell)| {
                if let Column::Context(key) = column {
                    Some((key.as_str(), cell.trim()))
                } else {
                    None
                }
            })
            .filter(|(_, value)| !value.is_empty())
            .collect::<Vec<_>>();
        let Some(mut variant) =
            variant_with_context(results, &self.title, &name, complexity_n, &context)
        else {
            return;
        };
        for ((column, _), cell) in columns.iter().zip(&cells) {
            let value = Finite::from(number(cell));
            match column {
                Column::Time(Some(factor)) => {
                    value.insert(&mut variant, &Latency::name_id(), "median", *factor);
                },
                Column::Time(None) => variant.skip_measure(),
                Column::Rate => insert_rate(&mut variant, &self.unit, value),
                Column::Counter(measure) => value.insert(&mut variant, measure, "median", 1.0),
                Column::Relative | Column::ComplexityN | Column::Context(_) | Column::Other => {},
            }
        }
    }

    /// The columns of a row of `cells`: the header's, with `complexityN` after any `relative`
    /// column, or without `complexityN`, since nanobench's header hash leaves `complexityN` out.
    /// nanobench pads every cell to its column's width and never cuts one, so a narrower cell, or
    /// a `complexityN` cell that is not a count, is a row that does not line up with these columns.
    fn columns(&self, cells: &[&str]) -> Option<Vec<&(Column, usize)>> {
        let count = cells.len();
        let mut columns = self.columns.iter().collect::<Vec<_>>();
        let complexity_n = columns
            .iter()
            .position(|(column, _)| matches!(column, Column::ComplexityN));
        if let Some(at) = complexity_n
            && count + 1 == columns.len()
        {
            columns.remove(at);
        } else if complexity_n.is_none() && count == columns.len() + 1 {
            let at = usize::from(matches!(columns.first(), Some((Column::Relative, _))));
            columns.insert(at, &(Column::ComplexityN, COMPLEXITY_N_WIDTH));
        }
        (count == columns.len()
            && columns.iter().zip(cells).all(|((column, width), cell)| {
                cell.len() + 1 >= *width
                    && (!matches!(column, Column::ComplexityN) || is_count(cell))
            }))
        .then_some(columns)
    }
}

const COMPLEXITY_N_WIDTH: usize = 14;

/// How many columns `rule` holds, if it is nanobench's rule under a header: right-aligned
/// columns, then the left-aligned title.
fn columns(rule: &str) -> Option<usize> {
    let (rule, title_rule) = rule.rsplit_once("|:")?;
    if !is_dashes(title_rule) {
        return None;
    }
    let mut count = 0;
    for cell in rule.strip_prefix('|')?.split('|') {
        if !cell.strip_suffix(':').is_some_and(is_dashes) {
            return None;
        }
        count += 1;
    }
    Some(count)
}

/// A column of nanobench's table.
enum Column {
    Relative,
    ComplexityN,
    /// The time per unit, with its factor to nanoseconds when the leaf knows its time unit.
    Time(Option<f64>),
    Rate,
    Counter(MeasureNameId),
    /// A `contextColumn()` variable, keyed by its name.
    Context(String),
    /// A column with no measure: `err%`, `IPC`, `miss%`, `total`, or `compare()`'s interval.
    Other,
}

impl Column {
    /// A column nanobench prints before the time, with its width.
    fn leading(title: &str) -> (Self, usize) {
        match title {
            "relative" => (Self::Relative, 11),
            "complexityN" => (Self::ComplexityN, COMPLEXITY_N_WIDTH),
            "95% CI" => (Self::Other, 22),
            _ => (Self::Context(title.to_owned()), (title.len() + 3).max(11)),
        }
    }

    /// A column nanobench prints after the rate, with its width, if nanobench prints it.
    fn trailing(title: &str, unit: &str) -> Option<(Self, usize)> {
        let counter = title
            .strip_suffix(unit)
            .and_then(|counter| counter.strip_suffix('/'));
        Some(match (counter, title) {
            (Some("ins"), _) => (Self::Counter(Instructions::name_id()), 18),
            (Some("cyc"), _) => (Self::Counter(Cycles::name_id()), 18),
            (Some("bra"), _) => (Self::Counter(Branches::name_id()), 17),
            (_, "err%" | "miss%") => (Self::Other, 10),
            (_, "IPC") => (Self::Other, 9),
            (_, "total") => (Self::Other, 12),
            _ => return None,
        })
    }
}

/// The factor to nanoseconds of a time unit nanobench's `timeUnit()` names.
fn nanoseconds(time_unit: &str) -> Option<f64> {
    match time_unit {
        "ns" => Some(1.0),
        "us" | "µs" | "μs" => Some(1e3),
        "ms" => Some(1e6),
        "s" => Some(1e9),
        _ => None,
    }
}

/// A row's cells before its name, and its name, or `None` for an error row: nanobench's
/// `MarkDownCode`, after any `:wavy_dash:`.
fn split(row: &str) -> Option<(Vec<&str>, String)> {
    let cells = row.strip_prefix('|')?.split('|').collect::<Vec<_>>();
    let start = cells.iter().position(|cell| {
        let cell = cell.trim_start();
        cell.starts_with('`') || cell.starts_with(":wavy_dash:") || cell.starts_with(":boom:")
    })?;
    let (cells, name) = cells.split_at(start);
    let name = name.join("|");
    let name = name.trim_start();
    let name = name.strip_prefix(":wavy_dash: ").unwrap_or(name);
    Some((cells.to_vec(), code(name)?))
}

/// The text of a markdown code span whose backticks nanobench doubles.
fn code(text: &str) -> Option<String> {
    let body = text.strip_prefix('`')?;
    let mut name = String::new();
    let mut chars = body.char_indices();
    while let Some((index, character)) = chars.next() {
        if character != '`' {
            name.push(character);
            continue;
        }
        if body.get(index + 1..)?.starts_with('`') {
            chars.next();
            name.push('`');
        } else {
            return Some(name);
        }
    }
    None
}

/// A number as nanobench prints it, with `,` between thousands.
fn number(cell: &str) -> Option<f64> {
    cell.trim().replace(',', "").parse().ok()
}

/// Whether `cell` is a whole number as nanobench prints it: right-aligned, its digits grouped by
/// 3 with `,` and no leading `0`, then the space after every cell.
fn is_count(cell: &str) -> bool {
    let Some(count) = cell.trim_start_matches(' ').strip_suffix(' ') else {
        return false;
    };
    if count == "0" {
        return true;
    }
    let mut groups = count.split(',');
    groups.next().is_some_and(|first| {
        (1..=3).contains(&first.len()) && !first.starts_with('0') && is_digits(first)
    }) && groups.all(|group| group.len() == 3 && is_digits(group))
}

fn is_digits(text: &str) -> bool {
    text.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_dashes(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte == b'-')
}

#[cfg(test)]
mod test_cpp_nanobench_text {
    use std::collections::BTreeSet;

    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{BytesThroughput, Cycles, Latency, Throughput},
                iai::Instructions,
                mitata::Branches,
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
        results::adapter_results::AdapterResults,
    };

    /// The fixture at `./tool_output/<suffix>` after `edit`, through the text leaf at version 1.
    fn convert_edited(suffix: &str, edit: impl FnOnce(String) -> String) -> AdapterResults {
        let input = std::fs::read_to_string(format!("./tool_output/{suffix}"))
            .unwrap_or_else(|e| panic!("Failed to read {suffix}: {e}"));
        Adapter::CppNanobenchText
            .convert(&edit(input), Settings::new(None, BmfVersion::V1))
            .unwrap_or_else(|| panic!("expected the text leaf to parse the edited {suffix}"))
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

    /// Kills a column read from its neighbour, a thousands separator that stops the number, a
    /// byte rate in `throughput`, `IPC`, `miss%`, `err%`, or `total` written, and nanobench's
    /// default title written as `@title`.
    #[test]
    fn adapter_cpp_nanobench_text_bitcoin() {
        let results = convert_v1(Adapter::CppNanobenchText, "cpp/nanobench/bitcoin_stdout.md");
        assert_eq!(results.resolved_adapter, Some(Adapter::CppNanobenchText));
        assert!(results.skipped.is_empty());
        assert_eq!(variants(&results), 10);
        assert_variant(
            &results,
            "Base58Decode",
            "{}",
            &[
                (Latency::SLUG_STR, vec![("median", 12.10)]),
                (BytesThroughput::SLUG_STR, vec![("median", 82_661_595.74)]),
                (Instructions::SLUG_STR, vec![("median", 284.56)]),
                (Cycles::SLUG_STR, vec![("median", 64.66)]),
                (Branches::SLUG_STR, vec![("median", 38.88)]),
            ],
        );
        assert_variant(
            &results,
            "AddrManAdd",
            "{}",
            &[
                (Latency::SLUG_STR, vec![("median", 16_588_900.0)]),
                (Throughput::SLUG_STR, vec![("median", 60.28)]),
                (Instructions::SLUG_STR, vec![("median", 293_109_543.0)]),
                (Cycles::SLUG_STR, vec![("median", 87_940_718.0)]),
                (Branches::SLUG_STR, vec![("median", 59_664_263.0)]),
            ],
        );
    }

    /// Kills a text reading that differs from the JSON of the same run beyond the table's
    /// rounding, such as a time or a counter per iteration where the JSON leaf reads it per unit.
    #[test]
    fn adapter_cpp_nanobench_text_agrees_with_json() {
        let text = convert_v1(Adapter::CppNanobenchText, "cpp/nanobench/bitcoin_stdout.md");
        let json = convert_v1(Adapter::CppNanobenchJson, "cpp/nanobench/bitcoin.json");
        let mut compared = 0;
        for (name, variants) in &text.inner {
            for (parameters, metrics) in variants {
                for (measure, metrics) in &metrics.inner {
                    for (metric, text_value) in &metrics.inner {
                        let json_value = value(
                            &json,
                            &name.to_string(),
                            &parameters.canonical(),
                            &measure.to_string(),
                            metric.as_ref(),
                        )
                        .unwrap_or_else(|| panic!("no {measure} {metric} of {name} in the JSON"));
                        assert!(
                            (text_value.into_inner() - json_value).abs() <= 0.005 + 1e-9,
                            "{name} {measure} {metric}: {text_value} != {json_value}"
                        );
                        compared += 1;
                    }
                }
            }
        }
        assert_eq!(compared, 50);
    }

    /// Kills a title the user set dropped, an unstable row dropped or named with its warning, a
    /// name cut at a `|` it holds, and the row whose `complexityN` cell its header lacks skipped,
    /// read one column off, or read without `@complexityN`.
    #[test]
    fn adapter_cpp_nanobench_text_titles_and_complexity() {
        let results = convert_v1(Adapter::CppNanobenchText, "cpp/nanobench/table.md");
        assert!(results.skipped.is_empty());
        assert_eq!(
            variant_names(&results),
            [
                (
                    "noisy | name with spaces",
                    r#"{"@complexityN":100,"@title":"Containers"}"#
                ),
                ("std::set insert", r#"{"@title":"Containers"}"#),
                ("std::string append", r#"{"@title":"Strings"}"#),
                ("std::vector push_back x1000", r#"{"@title":"Containers"}"#),
            ]
            .map(|(name, parameters)| (name.to_owned(), parameters.to_owned()))
            .into()
        );
        assert_variant(
            &results,
            "noisy | name with spaces",
            r#"{"@complexityN":100,"@title":"Containers"}"#,
            &[
                (Latency::SLUG_STR, vec![("median", 1_124.62)]),
                (Throughput::SLUG_STR, vec![("median", 889_190.16)]),
            ],
        );
        assert_value(
            &results,
            ("std::set insert", r#"{"@title":"Containers"}"#),
            (Latency::SLUG_STR, "median"),
            83.98,
        );
    }

    /// Kills an error row read or skipped uncounted, a repeated name that overwrites the first
    /// row or goes uncounted, a `compare()` table or its `95% CI` column misread, and the lines
    /// around the tables read as rows.
    #[test]
    fn adapter_cpp_nanobench_text_errors_repeats_and_compare() {
        let results = convert_v1(Adapter::CppNanobenchText, "cpp/nanobench/table_4_6_0.md");
        assert_eq!(
            (results.skipped.benchmarks, results.skipped.variants),
            (1, 1)
        );
        assert_eq!(
            variant_names(&results),
            [
                ("minstd", r#"{"@title":"Compare"}"#),
                ("mt19937", r#"{"@title":"Compare"}"#),
                ("std::set insert", r#"{"@title":"Containers"}"#),
                ("std::vector push_back x1000", r#"{"@title":"Containers"}"#),
                ("vector fill", r#"{"@title":"Context"}"#),
            ]
            .map(|(name, parameters)| (name.to_owned(), parameters.to_owned()))
            .into()
        );
        assert_value(
            &results,
            ("vector fill", r#"{"@title":"Context"}"#),
            (Latency::SLUG_STR, "median"),
            6.39,
        );
        assert_variant(
            &results,
            "minstd",
            r#"{"@title":"Compare"}"#,
            &[
                (Latency::SLUG_STR, vec![("median", 2.60)]),
                (Throughput::SLUG_STR, vec![("median", 384_823_923.70)]),
            ],
        );
    }

    /// Kills a time unit the user named read as nanoseconds, and one the leaf does not know
    /// read at all or skipped uncounted.
    #[test]
    fn adapter_cpp_nanobench_text_time_units() {
        let results = convert_edited("cpp/nanobench/table.md", |input| {
            input.replace("ns/insert", "ms/insert")
        });
        assert_value(
            &results,
            ("std::set insert", r#"{"@title":"Containers"}"#),
            (Latency::SLUG_STR, "median"),
            83_980_000.0,
        );
        let results = convert_edited("cpp/nanobench/table.md", |input| {
            input.replace("ns/insert", "ks/insert")
        });
        assert_eq!(results.skipped.measures, 3);
        assert_eq!(
            value(
                &results,
                "std::set insert",
                r#"{"@title":"Containers"}"#,
                Latency::SLUG_STR,
                "median"
            ),
            None
        );
    }

    /// Kills a backtick that nanobench doubles inside a name kept doubled, or taken as the end
    /// of the name.
    #[test]
    fn adapter_cpp_nanobench_text_backticks() {
        let results = convert_edited("cpp/nanobench/table.md", |input| {
            input.replace("`std::string append`", "`std::string ``append```")
        });
        assert_value(
            &results,
            ("std::string `append`", r#"{"@title":"Strings"}"#),
            (Latency::SLUG_STR, "median"),
            11.25,
        );
    }

    /// Kills the `complexityN` cell of a table with no `relative` column read one column off,
    /// as when nanobench adds it under a header without it.
    #[test]
    fn adapter_cpp_nanobench_text_complexity_without_relative() {
        let results = convert_edited("cpp/nanobench/table.md", |input| {
            input.replace(
                "|               11.25 |",
                "|          64 |                5.25 |       190,476,190.48 |    1.0% |      \
                 0.01 | `std::string reserve`\n|               11.25 |",
            )
        });
        assert_variant(
            &results,
            "std::string reserve",
            r#"{"@complexityN":64,"@title":"Strings"}"#,
            &[
                (Latency::SLUG_STR, vec![("median", 5.25)]),
                (Throughput::SLUG_STR, vec![("median", 190_476_190.48)]),
            ],
        );
    }

    /// Kills the row of a `Bench` with no `complexityN` under a header that has the column
    /// skipped, or read one column off.
    #[test]
    fn adapter_cpp_nanobench_text_complexity_row_short() {
        let results = convert_v1(
            Adapter::CppNanobenchText,
            "cpp/nanobench/table_4_6_0_complexity.md",
        );
        assert!(results.skipped.is_empty());
        assert_eq!(variants(&results), 2);
        assert_variant(
            &results,
            "with n",
            r#"{"@complexityN":100,"@title":"Sizes"}"#,
            &[
                (Latency::SLUG_STR, vec![("median", 9.15)]),
                (Throughput::SLUG_STR, vec![("median", 109_347_220.51)]),
            ],
        );
        assert_variant(
            &results,
            "without n",
            r#"{"@title":"Sizes"}"#,
            &[
                (Latency::SLUG_STR, vec![("median", 9.32)]),
                (Throughput::SLUG_STR, vec![("median", 107_267_132.27)]),
            ],
        );
    }

    /// Kills a row that keeps `complexityN` and drops `IPC`, 1 cell short of its header, read
    /// without its `complexityN` column, one column off.
    #[test]
    fn adapter_cpp_nanobench_text_complexity_row_without_ipc() {
        let results = convert_edited("cpp/nanobench/bitcoin_stdout.md", |input| {
            let (header, rule) = ("|               ns/op |", "|--------------------:");
            input
                .replacen(header, &format!("| complexityN {header}"), 1)
                .replacen(rule, &format!("|------------:{rule}"), 1)
                .replace(
                    "|       16,588,900.00 |               60.28 |    3.1% |  293,109,543.00 |   \
                     87,940,718.00 |  3.333 |",
                    "|         100 |       16,588,900.00 |               60.28 |    3.1% |  \
                     293,109,543.00 |   87,940,718.00 |",
                )
        });
        assert_eq!(
            (results.skipped.benchmarks, results.skipped.variants),
            (1, 0)
        );
        assert_eq!(variants(&results), 9);
        assert!(
            results
                .inner
                .keys()
                .all(|name| name.to_string() != "AddrManAdd")
        );
    }

    /// Kills a row that adds `complexityN` and drops `IPC`, or drops `complexityN` and adds
    /// `IPC`, read one column off: the same cell count as its header.
    #[test]
    fn adapter_cpp_nanobench_text_complexity_and_ipc() {
        for (suffix, name, parameters, median) in [
            (
                "cpp/nanobench/table_4_6_0_combo_add.md",
                "without n",
                r#"{"@title":"Sizes"}"#,
                0.19,
            ),
            (
                "cpp/nanobench/table_4_6_0_combo_drop.md",
                "with n",
                r#"{"@complexityN":100,"@title":"Sizes"}"#,
                0.10,
            ),
        ] {
            let results = convert_v1(Adapter::CppNanobenchText, suffix);
            assert_eq!(
                (results.skipped.benchmarks, results.skipped.variants),
                (1, 0),
                "{suffix}"
            );
            assert_eq!(
                variant_names(&results),
                [(name.to_owned(), parameters.to_owned())].into(),
                "{suffix}"
            );
            assert_value(
                &results,
                (name, parameters),
                (Latency::SLUG_STR, "median"),
                median,
            );
        }
    }

    /// Kills a `|` in a context name or value read as a column break, one column off, even
    /// after a 13-byte cell where `complexityN` goes.
    #[test]
    fn adapter_cpp_nanobench_text_context_pipes() {
        let results = convert_v1(
            Adapter::CppNanobenchText,
            "cpp/nanobench/table_4_6_0_pipe_after_13.md",
        );
        assert_eq!(
            (results.skipped.benchmarks, results.skipped.variants),
            (1, 0)
        );
        assert_eq!(
            variant_names(&results),
            [(
                "fill",
                r#"{"@title":"Threads","mode":"plain","num_threads":"4"}"#
            )]
            .map(|(name, parameters)| (name.to_owned(), parameters.to_owned()))
            .into()
        );
        for suffix in [
            "cpp/nanobench/table_4_6_0_pipe_13.md",
            "cpp/nanobench/table_4_6_0_pipe_name.md",
        ] {
            let results = convert_v1(Adapter::CppNanobenchText, suffix);
            assert_eq!(
                (results.skipped.benchmarks, results.skipped.variants),
                (2, 0),
                "{suffix}"
            );
            assert_eq!(variants(&results), 0, "{suffix}");
        }
    }

    /// Kills a `complexityN` cell wider than 13 bytes refused, either when the header has the
    /// column or when nanobench adds it under a header without it.
    #[test]
    fn adapter_cpp_nanobench_text_complexity_billion() {
        for (suffix, name, parameters, median) in [
            (
                "cpp/nanobench/table_4_6_0_big_n_first.md",
                "with big n",
                r#"{"@complexityN":1000000000,"@title":"Limits"}"#,
                8.51,
            ),
            (
                "cpp/nanobench/table_4_6_0_big_n_added.md",
                "with big n",
                r#"{"@complexityN":1000000000,"@title":"Limits"}"#,
                8.23,
            ),
            (
                "cpp/nanobench/table_4_6_0_pow_30.md",
                "with 2^30",
                r#"{"@complexityN":1073741824,"@title":"Limits"}"#,
                7.31,
            ),
        ] {
            let results = convert_v1(Adapter::CppNanobenchText, suffix);
            assert!(results.skipped.is_empty(), "{suffix}");
            assert_eq!(
                variant_names(&results),
                [
                    (name.to_owned(), parameters.to_owned()),
                    ("without n".to_owned(), r#"{"@title":"Limits"}"#.to_owned()),
                ]
                .into(),
                "{suffix}"
            );
            assert_value(
                &results,
                (name, parameters),
                (Latency::SLUG_STR, "median"),
                median,
            );
        }
    }

    /// Kills a context value split at its `|` read one column off when the cell where
    /// `complexityN` goes is wide enough but not a count as nanobench prints it (no `,` groups,
    /// a first group of 4 digits or with a leading `0`, a last group of 7, a letter for a digit,
    /// no space after it), and, in `count_9`, a context cell 1 byte narrower than its column read.
    #[test]
    fn adapter_cpp_nanobench_text_complexity_not_a_count() {
        for (suffix, name, parameters) in [
            (
                "cpp/nanobench/table_4_6_0_eq_pipe.md",
                "with n",
                r#"{"@complexityN":100,"@title":"Limits","mode":"x"}"#,
            ),
            (
                "cpp/nanobench/table_4_6_0_add_pipe.md",
                "plain",
                r#"{"@title":"Limits","mode":"x"}"#,
            ),
            (
                "cpp/nanobench/table_4_6_0_eq_9.md",
                "with n",
                r#"{"@complexityN":100,"@title":"Limits","mode":"x"}"#,
            ),
            (
                "cpp/nanobench/table_4_6_0_add_9.md",
                "plain",
                r#"{"@title":"Limits","mode":"x"}"#,
            ),
            (
                "cpp/nanobench/table_4_6_0_count_9.md",
                "plain",
                r#"{"@title":"Limits","mode":"x"}"#,
            ),
            (
                "cpp/nanobench/table_4_6_0_no_space.md",
                "plain",
                r#"{"@title":"Limits","mode":"x"}"#,
            ),
            (
                "cpp/nanobench/table_4_6_0_group_4.md",
                "plain",
                r#"{"@title":"Limits","mode":"x"}"#,
            ),
            (
                "cpp/nanobench/table_4_6_0_group_7.md",
                "plain",
                r#"{"@title":"Limits","mode":"x"}"#,
            ),
            (
                "cpp/nanobench/table_4_6_0_lead_0.md",
                "plain",
                r#"{"@title":"Limits","mode":"x"}"#,
            ),
            (
                "cpp/nanobench/table_4_6_0_letters.md",
                "plain",
                r#"{"@title":"Limits","mode":"x"}"#,
            ),
        ] {
            let results = convert_v1(Adapter::CppNanobenchText, suffix);
            assert_eq!(
                (results.skipped.benchmarks, results.skipped.variants),
                (1, 0),
                "{suffix}"
            );
            assert_eq!(
                variant_names(&results),
                [(name.to_owned(), parameters.to_owned())].into(),
                "{suffix}"
            );
        }
    }

    /// Kills a lone `0` refused as a `complexityN` cell: nanobench prints a `complexityN` that
    /// rounds to 0 as `0`.
    #[test]
    fn adapter_cpp_nanobench_text_complexity_zero() {
        let results = convert_v1(
            Adapter::CppNanobenchText,
            "cpp/nanobench/table_4_6_0_zero_n.md",
        );
        assert!(results.skipped.is_empty());
        assert_eq!(
            variant_names(&results),
            [
                (
                    "with n".to_owned(),
                    r#"{"@complexityN":100,"@title":"Limits"}"#.to_owned()
                ),
                (
                    "with n below 1".to_owned(),
                    r#"{"@title":"Limits"}"#.to_owned()
                ),
            ]
            .into()
        );
        assert_value(
            &results,
            ("with n below 1", r#"{"@title":"Limits"}"#),
            (Latency::SLUG_STR, "median"),
            7.38,
        );
    }

    /// Kills a row without `complexityN` under a header with it skipped when its first context
    /// cell is 13 bytes, the width of a `complexityN` cell.
    #[test]
    fn adapter_cpp_nanobench_text_complexity_short_context() {
        for (suffix, parameters, median) in [
            (
                "cpp/nanobench/table_4_6_0_value_12.md",
                r#"{"@title":"Limits","size":"123456789012"}"#,
                8.26,
            ),
            (
                "cpp/nanobench/table_4_6_0_name_11.md",
                r#"{"@title":"Limits","threadcount":"4"}"#,
                8.46,
            ),
        ] {
            let results = convert_v1(Adapter::CppNanobenchText, suffix);
            assert!(results.skipped.is_empty(), "{suffix}");
            assert_eq!(variants(&results), 2, "{suffix}");
            assert_value(
                &results,
                ("without n", parameters),
                (Latency::SLUG_STR, "median"),
                median,
            );
        }
    }

    /// Kills a context column dropped, which makes the second row a repeat, read as a measure,
    /// written as a number, or keyed with its padding.
    #[test]
    fn adapter_cpp_nanobench_text_context_columns() {
        let results = convert_v1(
            Adapter::CppNanobenchText,
            "cpp/nanobench/table_4_6_0_context.md",
        );
        assert!(results.skipped.is_empty());
        assert_eq!(
            variant_names(&results),
            [
                ("vector fill", r#"{"@title":"Context","size":"10"}"#),
                ("vector fill", r#"{"@title":"Context","size":"100"}"#),
            ]
            .map(|(name, parameters)| (name.to_owned(), parameters.to_owned()))
            .into()
        );
        assert_variant(
            &results,
            "vector fill",
            r#"{"@title":"Context","size":"100"}"#,
            &[
                (Latency::SLUG_STR, vec![("median", 16.60)]),
                (Throughput::SLUG_STR, vec![("median", 60_257_343.25)]),
            ],
        );
    }

    /// Kills a blank context cell, a variable the row does not set, written as an empty string,
    /// and context columns left out of the key cap.
    #[test]
    fn adapter_cpp_nanobench_text_context_key_cap() {
        let results = convert_v1(
            Adapter::CppNanobenchText,
            "cpp/nanobench/table_4_6_0_wide.md",
        );
        assert_eq!(
            (results.skipped.benchmarks, results.skipped.variants),
            (0, 1)
        );
        assert_eq!(variants(&results), 1);
        let mut parameters = serde_json::Map::new();
        parameters.insert("@title".to_owned(), "Wide".into());
        parameters.extend((0..15).map(|index| (format!("c{index}"), index.to_string().into())));
        assert_value(
            &results,
            (
                "fifteen",
                &serde_json::Value::Object(parameters).to_string(),
            ),
            (Latency::SLUG_STR, "median"),
            9.06,
        );
    }

    /// Kills a context column named for an adapter key that overwrites that key, or is skipped
    /// uncounted.
    #[test]
    fn adapter_cpp_nanobench_text_context_clash() {
        let results = convert_edited("cpp/nanobench/table_4_6_0_context.md", |input| {
            input.replace("|     size |", "|   @title |")
        });
        assert_eq!(
            (results.skipped.benchmarks, results.skipped.variants),
            (0, 3)
        );
        assert_value(
            &results,
            ("vector fill", r#"{"@title":"Context"}"#),
            (Latency::SLUG_STR, "median"),
            6.93,
        );
    }

    /// Kills a header whose last column is not nanobench's left-aligned title, such as a
    /// markdown table with a closing `|`, claimed as nanobench's.
    #[test]
    fn adapter_cpp_nanobench_text_closing_pipe() {
        let input = std::fs::read_to_string("./tool_output/cpp/nanobench/table.md")
            .expect("Failed to read a fixture");
        let closed = input
            .lines()
            .map(|line| format!("{line} |"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            Adapter::CppNanobenchText.convert(&closed, Settings::new(None, BmfVersion::V1)),
            None
        );
    }

    /// Kills a line a benchmark prints between two rows that closes the table, dropping the
    /// rows after it uncounted.
    #[test]
    fn adapter_cpp_nanobench_text_print_between_rows() {
        let results = convert_edited("cpp/nanobench/bitcoin_stdout.md", |input| {
            input.replacen(
                "\n|                1.77 |",
                "\nselected the SHANI implementation\n|                1.77 |",
                1,
            )
        });
        assert!(results.skipped.is_empty());
        assert_eq!(variants(&results), 10);
    }

    /// Kills the rows under a header the leaf cannot read dropped uncounted, or read with the
    /// columns of the table before it, which give a byte rate to `throughput`.
    #[test]
    fn adapter_cpp_nanobench_text_unread_header() {
        let results = convert_edited("cpp/nanobench/bitcoin_stdout.md", |input| {
            input.replacen("ns/byte |", "ns/word |", 1)
        });
        assert_eq!(results.skipped.benchmarks, 2);
        assert_eq!(variants(&results), 8);
        assert!(
            results
                .inner
                .keys()
                .all(|name| name.to_string() != "Base58Decode")
        );
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_cpp_nanobench_text_ignores_average() {
        for suffix in ["cpp/nanobench/bitcoin_stdout.md", "cpp/nanobench/table.md"] {
            assert_ignores_average(Adapter::CppNanobenchText, suffix);
        }
    }
}
