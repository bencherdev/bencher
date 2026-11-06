use bencher_json::{
    BenchmarkName, ParameterSet,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{CpuTime, Latency, SystemTime, UserTime},
        },
        report::Adapter,
    },
};

use crate::{
    Adaptable, Settings,
    adapters::{bmf_v1::leaf::LeafResults, util::Units},
    results::adapter_results::AdapterResults,
};

/// Ruby's `Benchmark` report at version 1: each row under a `user system total real` header, until
/// an empty line or a rehearsal, is one measurement of each time, its `value`.
///
/// A line that is not a row leaves the table open, since a block that prints splits its row.
pub struct AdapterRubyBenchmarkV1;

impl Adaptable for AdapterRubyBenchmarkV1 {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut results = LeafResults::default();
        let mut header = false;
        for line in input.lines() {
            if is_header(line) {
                header = true;
                continue;
            }
            if !header {
                continue;
            }
            let Some(Row {
                label_and_user,
                system,
                total,
                real,
            }) = parse_row(line)
            else {
                if closes_table(line) {
                    header = false;
                }
                continue;
            };
            let Some((label, user)) = label_and_user else {
                results.skip_benchmark();
                continue;
            };
            let Ok(benchmark_name) = label.parse::<BenchmarkName>() else {
                results.skip_benchmark();
                continue;
            };
            let Some(mut variant) = results.new_variant(benchmark_name, ParameterSet::default())
            else {
                continue;
            };
            for (measure, seconds) in [
                (Latency::name_id(), real),
                (UserTime::name_id(), user),
                (SystemTime::name_id(), system),
                (CpuTime::name_id(), total),
            ] {
                variant.insert(measure, "value", seconds * Units::Sec.as_nanos());
            }
        }
        results.finish_with_skips(Adapter::RubyBenchmark)
    }
}

/// A block's output with no newline can start the header line, since benchmark 0.5.0 runs every
/// block before it prints the table.
fn is_header(line: &str) -> bool {
    line.split_whitespace()
        .rev()
        .take(4)
        .eq(["real", "total", "system", "user"])
}

/// An empty line, or the first line of a `bmbm` rehearsal, which a block's output can start.
fn closes_table(line: &str) -> bool {
    line.is_empty() || line.trim_end_matches('-').ends_with("Rehearsal ")
}

/// One row: a label, then the user, system, and total times, and the real time in parentheses.
struct Row<'a> {
    /// `None` when a user time of 100 s or more fills its column and glues the label to it.
    label_and_user: Option<(&'a str, f64)>,
    system: f64,
    total: f64,
    real: f64,
}

fn parse_row(line: &str) -> Option<Row<'_>> {
    let (head, real) = line.strip_suffix(')')?.rsplit_once('(')?;
    let real = seconds(real.trim())?;
    let (rest, total) = last_time(head)?;
    let (rest, system) = last_time(rest)?;
    Some(Row {
        label_and_user: last_time(rest),
        system,
        total,
        real,
    })
}

fn last_time(text: &str) -> Option<(&str, f64)> {
    let (before, number) = text.trim_end().rsplit_once(' ')?;
    Some((before.trim_end(), seconds(number)?))
}

fn seconds(number: &str) -> Option<f64> {
    number
        .parse()
        .ok()
        .filter(|seconds: &f64| seconds.is_finite())
}

#[cfg(test)]
mod test_ruby_benchmark_v1 {
    use bencher_json::project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{CpuTime, Latency, SystemTime, UserTime},
        },
        report::Adapter,
    };
    use pretty_assertions::assert_eq;

    use crate::adapters::bmf_v1::test_util::{
        Expected, assert_ignores_average, assert_variant, convert_v1, variants,
    };

    /// A row's four times in seconds: user, system, total, and real.
    fn times(user: f64, system: f64, total: f64, real: f64) -> Vec<Expected> {
        vec![
            (Latency::SLUG_STR, vec![("value", real * 1e9)]),
            (UserTime::SLUG_STR, vec![("value", user * 1e9)]),
            (SystemTime::SLUG_STR, vec![("value", system * 1e9)]),
            (CpuTime::SLUG_STR, vec![("value", total * 1e9)]),
        ]
    }

    /// Kills a column read into the wrong measure, a time left in seconds, a time stored as a
    /// statistic rather than `value`, a label cut at its space, a label that repeats one already
    /// read stored over it or left uncounted, a row with no label dropped uncounted, and a leaf
    /// that records no leaf.
    #[test]
    fn adapter_ruby_benchmark_v1_bm() {
        let results = convert_v1(Adapter::RubyBenchmark, "ruby/benchmark/bm.txt");
        assert_eq!(results.resolved_adapter, Some(Adapter::RubyBenchmark));
        for (name, expected) in [
            ("for:", times(0.007_336, 0.0, 0.007_336, 0.007_333)),
            ("times:", times(0.006_462, 0.0, 0.006_462, 0.006_464)),
            ("up to:", times(0.006_813, 0.0, 0.006_813, 0.006_813)),
        ] {
            assert_variant(&results, name, "{}", &expected);
        }
        assert_eq!(variants(&results), 3);
        assert_eq!(results.skipped.variants, 1);
        assert_eq!(results.skipped.benchmarks, 1);
    }

    /// Kills the table closed at the line a block prints between its label and its times, which
    /// drops every row after it uncounted, and the times left without their label stored or left
    /// uncounted.
    #[test]
    fn adapter_ruby_benchmark_v1_loud() {
        let results = convert_v1(Adapter::RubyBenchmark, "ruby/benchmark/loud.txt");
        for (name, expected) in [
            ("one:", times(0.003_479, 0.000_064, 0.003_543, 0.003_541)),
            ("two:", times(0.003_240, 0.0, 0.003_240, 0.003_240)),
            ("three:", times(0.003_217, 0.0, 0.003_217, 0.003_217)),
        ] {
            assert_variant(&results, name, "{}", &expected);
        }
        assert_eq!(variants(&results), 3);
        assert_eq!(results.skipped.benchmarks, 1);
        assert_eq!(results.skipped.variants, 0);
    }

    /// Kills the table closed at a row labelled `total:` whose block prints a line, which drops
    /// every row after it uncounted.
    #[test]
    fn adapter_ruby_benchmark_v1_label_total() {
        let results = convert_v1(Adapter::RubyBenchmark, "ruby/benchmark/label_total.txt");
        for (name, expected) in [
            ("one:", times(0.003_322, 0.000_125, 0.003_447, 0.003_446)),
            ("two:", times(0.003_223, 0.0, 0.003_223, 0.003_224)),
        ] {
            assert_variant(&results, name, "{}", &expected);
        }
        assert_eq!(variants(&results), 2);
        assert_eq!(results.skipped.benchmarks, 1);
        assert_eq!(results.skipped.variants, 0);
    }

    /// Kills the table closed at a line of spaces, the empty label of a row whose block prints a
    /// newline, which drops every row after it uncounted.
    #[test]
    fn adapter_ruby_benchmark_v1_empty_label() {
        let results = convert_v1(Adapter::RubyBenchmark, "ruby/benchmark/empty_label.txt");
        for (name, expected) in [
            ("one:", times(0.003_484, 0.0, 0.003_484, 0.003_482)),
            ("two:", times(0.004_039, 0.0, 0.004_039, 0.004_036)),
        ] {
            assert_variant(&results, name, "{}", &expected);
        }
        assert_eq!(variants(&results), 2);
        assert_eq!(results.skipped.benchmarks, 1);
        assert_eq!(results.skipped.variants, 0);
    }

    /// Kills a row dropped uncounted when a user time of 100 s or more fills its column and
    /// glues the label to it.
    #[test]
    fn adapter_ruby_benchmark_v1_slow() {
        let results = convert_v1(Adapter::RubyBenchmark, "ruby/benchmark/slow.txt");
        for (name, expected) in [
            ("one:", times(0.009_990, 0.000_086, 0.010_076, 0.010_075)),
            ("two:", times(0.010_687, 0.0, 0.010_687, 0.010_695)),
        ] {
            assert_variant(&results, name, "{}", &expected);
        }
        assert_eq!(variants(&results), 2);
        assert_eq!(results.skipped.benchmarks, 1);
        assert_eq!(results.skipped.variants, 0);
    }

    /// Kills a table left open past a `Rehearsal` line that a block's output starts, which reads
    /// the rehearsal and makes the real rows repeats, and a table closed at any line holding
    /// `Rehearsal `, which drops the rows after a block that prints it.
    #[test]
    fn adapter_ruby_benchmark_v1_setup() {
        let results = convert_v1(Adapter::RubyBenchmark, "ruby/benchmark/setup.txt");
        for (name, expected) in [
            ("one:", times(0.003_868, 0.0, 0.003_868, 0.003_871)),
            ("two:", times(0.003_350, 0.0, 0.003_350, 0.003_352)),
            ("sort!", times(0.002_942, 0.0, 0.002_942, 0.002_943)),
            ("sort", times(0.002_641, 0.0, 0.002_641, 0.002_642)),
        ] {
            assert_variant(&results, name, "{}", &expected);
        }
        assert_eq!(variants(&results), 4);
        assert_eq!(results.skipped.benchmarks, 1);
        assert_eq!(results.skipped.variants, 0);
    }

    /// Kills a header that is any line ending in `real` or in `total real`, which opens the table
    /// at a rehearsal line a block prints and makes the real rows repeats.
    #[test]
    fn adapter_ruby_benchmark_v1_caption() {
        let results = convert_v1(Adapter::RubyBenchmark, "ruby/benchmark/caption.txt");
        for (name, expected) in [
            ("one:", times(0.003_362, 0.0, 0.003_362, 0.003_363)),
            ("two:", times(0.003_128, 0.0, 0.003_128, 0.003_128)),
        ] {
            assert_variant(&results, name, "{}", &expected);
        }
        assert_eq!(variants(&results), 2);
        assert_eq!(results.skipped.benchmarks, 1);
        assert_eq!(results.skipped.variants, 0);
    }

    /// Kills a header that must be the whole line: benchmark 0.5.0 runs every block before it
    /// prints the table, so a block's output with no newline starts the header line.
    #[test]
    fn adapter_ruby_benchmark_v1_noisy() {
        let results = convert_v1(Adapter::RubyBenchmark, "ruby/benchmark/noisy.txt");
        for (name, expected) in [
            ("one:", times(0.003_026, 0.000_018, 0.003_044, 0.003_049)),
            ("noisy:", times(0.002_788, 0.0, 0.002_788, 0.002_793)),
            ("two:", times(0.002_763, 0.0, 0.002_763, 0.002_768)),
            ("three:", times(0.002_648, 0.000_061, 0.002_709, 0.002_713)),
        ] {
            assert_variant(&results, name, "{}", &expected);
        }
        assert_eq!(variants(&results), 4);
        assert!(results.skipped.is_empty());
    }

    /// Kills a table left open past the `Rehearsal` line of a `bmbm` right after it, which reads
    /// the rehearsal and makes the real rows repeats, or past a blank line, which reads the
    /// `Benchmark.measure` row printed after it. The row printed right after a table is read.
    #[test]
    fn adapter_ruby_benchmark_v1_tables() {
        let results = convert_v1(Adapter::RubyBenchmark, "ruby/benchmark/tables.txt");
        for (name, expected) in [
            ("one:", times(0.003_371, 0.000_089, 0.003_460, 0.003_457)),
            ("hand:", times(0.003_140, 0.000_030, 0.003_170, 0.003_171)),
            ("sort!", times(0.002_781, 0.0, 0.002_781, 0.002_776)),
            ("sort", times(0.002_656, 0.0, 0.002_656, 0.002_656)),
        ] {
            assert_variant(&results, name, "{}", &expected);
        }
        assert_eq!(variants(&results), 4);
        assert!(results.skipped.is_empty());
    }

    /// Kills the rehearsal read, whose rows come first and would make the real rows repeats.
    #[test]
    fn adapter_ruby_benchmark_v1_bmbm() {
        let results = convert_v1(Adapter::RubyBenchmark, "ruby/benchmark/bmbm.txt");
        assert!(results.skipped.is_empty());
        assert_variant(
            &results,
            "sort!",
            "{}",
            &times(0.017_336, 0.000_022, 0.017_358, 0.017_359),
        );
        assert_variant(
            &results,
            "sort",
            "{}",
            &times(0.017_331, 0.000_015, 0.017_346, 0.017_346),
        );
    }

    /// Kills the rows `Benchmark.benchmark` prints for the labels it is given after the
    /// reports dropped.
    #[test]
    fn adapter_ruby_benchmark_v1_totals() {
        let results = convert_v1(Adapter::RubyBenchmark, "ruby/benchmark/totals.txt");
        assert_eq!(variants(&results), 5);
        assert_variant(
            &results,
            ">total:",
            "{}",
            &times(0.019_748, 0.000_980, 0.020_728, 0.020_728),
        );
        assert_variant(
            &results,
            ">avg:",
            "{}",
            &times(0.006_583, 0.000_327, 0.006_909, 0.006_909),
        );
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_ruby_benchmark_v1_ignores_average() {
        for suffix in [
            "ruby/benchmark/bm.txt",
            "ruby/benchmark/bmbm.txt",
            "ruby/benchmark/caption.txt",
            "ruby/benchmark/empty_label.txt",
            "ruby/benchmark/five.txt",
            "ruby/benchmark/label_total.txt",
            "ruby/benchmark/loud.txt",
            "ruby/benchmark/noisy.txt",
            "ruby/benchmark/setup.txt",
            "ruby/benchmark/slow.txt",
            "ruby/benchmark/tables.txt",
            "ruby/benchmark/totals.txt",
            "ruby/benchmark/two.txt",
        ] {
            assert_ignores_average(Adapter::RubyBenchmark, suffix);
        }
    }
}
