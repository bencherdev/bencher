use bencher_json::{
    BenchmarkName, MeasureNameId, ParameterSet,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Allocations, BytesThroughput, Latency, Throughput},
            divan::{
                CharsThroughput, CyclesThroughput, Deallocated, Deallocations, Grown, Grows,
                MaxAllocated, MaxAllocations, Shrinks, Shrunk,
            },
            dotnet::Allocated,
        },
        report::Adapter,
    },
};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::leaf::{LeafResults, Variant},
    results::adapter_results::AdapterResults,
};

/// Divan's tree, read row by row as its printer writes it under each header: every benchmark is
/// named by Divan's own `::` path, from the crate the header names down to its row.
///
/// A line inside a tree that the printer did not place (a benchmark's own output, a newline in an
/// argument, a panic's unfinished row) is skipped and counted, and so is every line outside a tree
/// but libtest's preamble. An `(ignored)` row is no benchmark and no count. Text without a header
/// is not Divan's.
pub struct AdapterRustDivan;

impl Adaptable for AdapterRustDivan {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut reader = Reader::default();
        for line in input.lines() {
            reader.read(line);
        }
        if !reader.has_header {
            return None;
        }
        reader.results.finish_with_skips(Adapter::RustDivan)
    }
}

#[derive(Default)]
struct Reader {
    results: LeafResults,
    tree: Option<Tree>,
    has_header: bool,
}

impl Reader {
    fn read(&mut self, line: &str) {
        if let Some((root, glued)) = header(line) {
            if glued {
                self.results.skip_benchmark();
            }
            self.tree = Some(Tree::new(root));
            self.has_header = true;
            return;
        }
        if line.is_empty() {
            self.tree = None;
            return;
        }
        let placed = match &mut self.tree {
            Some(tree) => tree.read(line, &mut self.results),
            None => is_preamble(line),
        };
        if !placed {
            self.results.skip_benchmark();
        }
    }
}

/// The crate a tree's header names, and whether other output came before the header on its line,
/// as a panicking benchmark's unfinished row leaves it.
fn header(line: &str) -> Option<(&str, bool)> {
    let (head, columns) = cells(line)?;
    if columns.map(str::trim) != ["slowest", "median", "mean", "samples", "iters"] {
        return None;
    }
    let names = head.trim_end().strip_suffix("fastest")?.trim_end();
    let (glued, root) = names.rsplit_once(' ').unwrap_or(("", names));
    (!root.is_empty()).then_some((root, !glued.trim().is_empty()))
}

/// libtest's lines for a target's tests, which `cargo bench` prints before a tree.
fn is_preamble(line: &str) -> bool {
    let running = line
        .strip_prefix("running ")
        .and_then(|rest| {
            rest.strip_suffix(" tests")
                .or_else(|| rest.strip_suffix(" test"))
        })
        .is_some_and(is_count);
    let ignored = line.starts_with("test ")
        && (line.ends_with(" ... ignored") || line.contains(" ... ignored, "));
    running || ignored || line.starts_with("test result: ")
}

/// One tree: the crate its header names, its open parent rows, and the benchmark of its last row.
struct Tree {
    root: String,
    parents: Vec<Parent>,
    benchmark: Option<Benchmark>,
}

impl Tree {
    fn new(root: &str) -> Self {
        Self {
            root: root.to_owned(),
            parents: Vec::new(),
            benchmark: None,
        }
    }

    /// Reads a line of the tree, or returns `false` if the printer did not place it.
    fn read(&mut self, line: &str, results: &mut LeafResults) -> bool {
        if let Some((depth, last, row)) = self.branch(line) {
            return self.read_row(depth, last, row, results);
        }
        self.benchmark
            .as_mut()
            .is_some_and(|benchmark| benchmark.read(line, results))
    }

    /// A row's depth, whether it is its parent's last, and the row after its branch, if the line
    /// starts with the prefix of the open parents down to some depth and then a branch.
    fn branch<'l>(&self, line: &'l str) -> Option<(usize, bool, &'l str)> {
        let mut rest = line;
        let mut depth = 0;
        loop {
            if let Some(row) = rest.strip_prefix("├─ ") {
                return Some((depth, false, row));
            }
            if let Some(row) = rest.strip_prefix("╰─ ") {
                return Some((depth, true, row));
            }
            rest = rest.strip_prefix(self.parents.get(depth)?.prefix())?;
            depth += 1;
        }
    }

    fn read_row(&mut self, depth: usize, last: bool, row: &str, results: &mut LeafResults) -> bool {
        self.benchmark = None;
        let Some(row) = cells(row).and_then(|(head, cells)| Row::of(head, cells)) else {
            return false;
        };
        self.parents.truncate(depth);
        match row {
            Row::Parent(name) => self.parents.push(Parent {
                name: name.to_owned(),
                last,
            }),
            Row::Leaf(name, times) => {
                let name = self.new_benchmark(name, times, results);
                let mut prefix: String = self.parents.iter().map(Parent::prefix).collect();
                if !last {
                    prefix.push('│');
                }
                self.benchmark = Some(Benchmark {
                    name,
                    prefix,
                    next: Next::Rate,
                });
            },
        }
        true
    }

    /// The benchmark a leaf row names, with its times, or `None`, counted as skipped, if the name
    /// is not valid or the tree already named it.
    fn new_benchmark(
        &self,
        name: &str,
        times: [f64; 4],
        results: &mut LeafResults,
    ) -> Option<BenchmarkName> {
        let mut path = self.root.clone();
        for segment in self
            .parents
            .iter()
            .map(|parent| parent.name.as_str())
            .chain([name])
        {
            path.push_str("::");
            path.push_str(segment);
        }
        let Ok(name) = path.parse::<BenchmarkName>() else {
            results.skip_benchmark();
            return None;
        };
        let mut variant = results.new_variant(name.clone(), ParameterSet::default())?;
        for (metric, time) in ["min", "max", "median", "mean"].into_iter().zip(times) {
            variant.insert(Latency::name_id(), metric, time);
        }
        Some(name)
    }
}

/// A parent row, and whether it is its own parent's last.
struct Parent {
    name: String,
    last: bool,
}

impl Parent {
    /// What the printer writes before each row below this one.
    fn prefix(&self) -> &'static str {
        if self.last { "   " } else { "│  " }
    }
}

/// A row after its branch. An `(ignored)` row holds no times, as a parent row does, and no row
/// is ever below it.
enum Row<'l> {
    Parent(&'l str),
    /// A benchmark's name, and its fastest, slowest, median, and mean times in nanoseconds.
    Leaf(&'l str, [f64; 4]),
}

impl<'l> Row<'l> {
    /// The row that a name, its padding, and its first cell make with the five cells after them.
    fn of(head: &'l str, cells: [&str; 5]) -> Option<Self> {
        let head = head.trim_end();
        if cells.iter().all(|cell| cell.trim().is_empty()) {
            return Some(Self::Parent(head));
        }
        let [slowest, median, mean, samples, iters] = cells;
        if !is_count(samples.trim()) || !is_count(iters) {
            return None;
        }
        let (rest, unit) = head.rsplit_once(' ')?;
        let (name, fastest) = rest.rsplit_once(' ').unwrap_or(("", rest));
        Some(Self::Leaf(
            name.trim_end(),
            every([
                time(fastest, unit),
                time_cell(slowest),
                time_cell(median),
                time_cell(mean),
            ])?,
        ))
    }
}

/// The benchmark of a tree's last row, `None` when it was skipped, with where the rows under its
/// times start and which of them comes next.
struct Benchmark {
    name: Option<BenchmarkName>,
    prefix: String,
    next: Next,
}

impl Benchmark {
    /// Reads one of the rows the printer writes under a benchmark's times, or returns `false` if
    /// the line is not the next such row.
    fn read(&mut self, line: &str, results: &mut LeafResults) -> bool {
        let Some((head, [slowest, median, mean, samples, iters])) =
            line.strip_prefix(self.prefix.as_str()).and_then(cells)
        else {
            return false;
        };
        if !samples.trim().is_empty() || !iters.is_empty() {
            return false;
        }
        let Some((columns, next)) = self.next.read([head, slowest, median, mean].map(str::trim))
        else {
            return false;
        };
        if let (Some(name), Some((measure, values))) = (&self.name, columns) {
            let mut variant = results.variant(name.clone(), ParameterSet::default());
            insert(&mut variant, measure, values);
        }
        self.next = next;
        true
    }
}

/// The row under a benchmark's times that the printer can write next: a rate or a tally's label,
/// a tally's count, or a tally's bytes.
#[derive(Clone, Copy)]
enum Next {
    Rate,
    Count(Tally),
    Bytes(Tally),
}

impl Next {
    /// The measure and the values a row's four columns hold, if any, and the row after it, or
    /// `None` if the row cannot come next.
    fn read(self, columns: [&str; 4]) -> Option<(Option<Values>, Self)> {
        match self {
            Self::Rate => {
                if let [label, "", "", ""] = columns
                    && let Some(tally) = Tally::of(label)
                {
                    return Some((None, Self::Count(tally)));
                }
                let [(kind, fastest), (_, slowest), (_, median), (_, mean)] =
                    every(columns.map(rate))?;
                let values = [fastest, slowest, median, mean];
                Some((Some((Measure::Rate(kind), values)), Self::Rate))
            },
            Self::Count(tally) => Some((
                Some((Measure::Count(tally), every(columns.map(number))?)),
                Self::Bytes(tally),
            )),
            Self::Bytes(tally) => Some((
                Some((Measure::Bytes(tally), every(columns.map(bytes))?)),
                Self::Rate,
            )),
        }
    }
}

/// A row's measure and its four values.
type Values = (Measure, [f64; 4]);

/// What one row under a benchmark's times measures.
#[derive(Clone, Copy)]
enum Measure {
    Rate(Rate),
    Count(Tally),
    Bytes(Tally),
}

/// A row's four columns under Divan's labels. A rate is infinite at a time of zero, which the
/// times already hold, so it is skipped uncounted.
fn insert(variant: &mut Variant<'_>, measure: Measure, values: [f64; 4]) {
    let measure = match measure {
        Measure::Rate(rate) => rate.measure(),
        Measure::Count(tally) => tally.count(),
        Measure::Bytes(tally) => {
            let Some(measure) = tally.bytes(variant) else {
                return;
            };
            measure
        },
    };
    for (metric, value) in ["fastest", "slowest", "median", "mean"]
        .into_iter()
        .zip(values)
    {
        if value.is_finite() {
            variant.insert(measure.clone(), metric, value);
        }
    }
}

/// What a rate counts.
#[derive(Clone, Copy)]
enum Rate {
    Bytes,
    Chars,
    Cycles,
    Items,
}

impl Rate {
    /// The rate a unit is of, and the factor from the unit to one per second.
    fn of(unit: &str) -> Option<(Self, f64)> {
        [
            (Self::Bytes, "B/s"),
            (Self::Chars, "char/s"),
            (Self::Cycles, "Hz"),
            (Self::Items, "item/s"),
        ]
        .into_iter()
        .find_map(|(rate, base)| Some((rate, scale(unit.strip_suffix(base)?)?)))
    }

    fn measure(self) -> MeasureNameId {
        match self {
            Self::Bytes => BytesThroughput::name_id(),
            Self::Chars => CharsThroughput::name_id(),
            Self::Cycles => CyclesThroughput::name_id(),
            Self::Items => Throughput::name_id(),
        }
    }
}

/// One of the allocator's tallies Divan prints, each a count and a number of bytes.
#[derive(Clone, Copy)]
enum Tally {
    MaxAlloc,
    Alloc,
    Dealloc,
    Grow,
    Shrink,
}

impl Tally {
    fn of(label: &str) -> Option<Self> {
        match label {
            "max alloc:" => Some(Self::MaxAlloc),
            "alloc:" => Some(Self::Alloc),
            "dealloc:" => Some(Self::Dealloc),
            "grow:" => Some(Self::Grow),
            "shrink:" => Some(Self::Shrink),
            _ => None,
        }
    }

    fn count(self) -> MeasureNameId {
        match self {
            Self::MaxAlloc => MaxAllocations::name_id(),
            Self::Alloc => Allocations::name_id(),
            Self::Dealloc => Deallocations::name_id(),
            Self::Grow => Grows::name_id(),
            Self::Shrink => Shrinks::name_id(),
        }
    }

    fn bytes(self, variant: &mut Variant<'_>) -> Option<MeasureNameId> {
        match self {
            Self::MaxAlloc => Some(MaxAllocated::name_id()),
            Self::Alloc => variant.named(Allocated::SLUG_STR, Allocated::UNITS_STR),
            Self::Dealloc => Some(Deallocated::name_id()),
            Self::Grow => Some(Grown::name_id()),
            Self::Shrink => Some(Shrunk::name_id()),
        }
    }
}

/// The text before a row's last five cells, and those cells, split from the right, since a name
/// can hold the separator and a column can widen in the middle of a tree. The last cell is empty
/// where the line ends in its separator.
fn cells(line: &str) -> Option<(&str, [&str; 5])> {
    let (line, iters) = if let Some(line) = line.strip_suffix(" │") {
        (line, "")
    } else {
        line.rsplit_once(" │ ")?
    };
    let (line, samples) = line.rsplit_once(" │ ")?;
    let (line, mean) = line.rsplit_once(" │ ")?;
    let (line, median) = line.rsplit_once(" │ ")?;
    let (head, slowest) = line.rsplit_once(" │ ")?;
    Some((head, [slowest, median, mean, samples, iters]))
}

/// A time in nanoseconds, from each unit Divan prints.
fn time(number: &str, unit: &str) -> Option<f64> {
    let nanos = match unit {
        "ns" => 1.0,
        "µs" => 1e3,
        "ms" => 1e6,
        "s" => 1e9,
        "m" => 60e9,
        "h" => 3_600e9,
        "d" => 86_400e9,
        _ => return None,
    };
    Some(self::number(number)? * nanos)
}

fn time_cell(cell: &str) -> Option<f64> {
    let (number, unit) = cell.trim().split_once(' ')?;
    time(number, unit)
}

/// A rate per second and what it counts, infinite where Divan prints `inf`.
fn rate(cell: &str) -> Option<(Rate, f64)> {
    let (number, unit) = cell.split_once(' ')?;
    let (rate, factor) = Rate::of(unit)?;
    if number == "inf" {
        return Some((rate, f64::INFINITY));
    }
    Some((rate, self::number(number)? * factor))
}

/// A number of bytes.
fn bytes(cell: &str) -> Option<f64> {
    let (number, unit) = cell.split_once(' ')?;
    Some(self::number(number)? * scale(unit.strip_suffix('B')?)?)
}

/// The factor of a decimal or binary prefix, up to peta.
fn scale(prefix: &str) -> Option<f64> {
    let power = |prefixes: [&str; 6], base: f64| {
        prefixes
            .into_iter()
            .zip(0..)
            .find_map(|(other, power)| (other == prefix).then(|| base.powi(power)))
    };
    power(["", "K", "M", "G", "T", "P"], 1_000.0)
        .or_else(|| power(["", "Ki", "Mi", "Gi", "Ti", "Pi"], 1_024.0))
}

/// A number as Divan prints one, in plain decimal.
fn number(text: &str) -> Option<f64> {
    text.bytes()
        .all(|byte| byte.is_ascii_digit() || byte == b'.')
        .then(|| text.parse().ok())
        .flatten()
}

fn is_count(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// Every value, if each is there.
fn every<T>(values: [Option<T>; 4]) -> Option<[T; 4]> {
    let [Some(first), Some(second), Some(third), Some(fourth)] = values else {
        return None;
    };
    Some([first, second, third, fourth])
}

#[cfg(test)]
mod test_rust_divan {
    use std::collections::BTreeSet;

    use bencher_json::project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Allocations, BytesThroughput, Latency, Throughput},
            divan::{
                CharsThroughput, CyclesThroughput, Deallocated, Deallocations, Grown, Grows,
                MaxAllocated, MaxAllocations, Shrinks, Shrunk,
            },
            dotnet::Allocated,
        },
        report::Adapter,
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_units, assert_variant, convert_v1, variants,
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    use super::AdapterRustDivan;

    /// Kills a reader that names a benchmark by anything but its whole `::` path, trims a name's
    /// leading spaces, splits a row from the left where a name holds ` │ `, reads a name that starts
    /// with a branch as a deeper row, reads a `t=N` row as a parameter, or reads the lines of a
    /// benchmark's own output and of an argument's newline instead of counting each.
    #[test]
    fn adapter_rust_divan_edge() {
        let results = convert_v1(Adapter::RustDivan, "rust/divan/edge_default.txt");
        assert_eq!(results.resolved_adapter, Some(Adapter::RustDivan));
        assert_reading(&results, &edge(), skipped(4), &[]);
    }

    /// Kills a reader that reads a row at fixed offsets, which a row wider than the header breaks.
    #[test]
    fn adapter_rust_divan_wide_row() {
        let results = convert_v1(Adapter::RustDivan, "rust/divan/ovf_default.txt");
        assert_reading(&results, &ovf(), skipped(0), &[]);
    }

    /// Kills a reader that sends a tally's count or bytes to another measure, swaps them, scales a
    /// decimal size wrong, drops a fractional size, keeps only some of a row's columns, or gives
    /// allocated bytes no unit.
    #[test]
    fn adapter_rust_divan_allocations() {
        let results = convert_v1(Adapter::RustDivan, "rust/divan/alloc_default.txt");
        assert_reading(&results, &alloc(), skipped(0), &[ALLOCATED_UNIT]);
    }

    /// Kills a reader that drops deallocations, a fractional count, a parent's prefix of spaces
    /// under a last row, or a name with a space.
    #[test]
    fn adapter_rust_divan_more() {
        let results = convert_v1(Adapter::RustDivan, "rust/divan/more_default.txt");
        assert_reading(&results, &more(), skipped(0), &[ALLOCATED_UNIT]);
    }

    /// Kills a reader that scales a binary size or rate as decimal, or the other way around, stops
    /// short of peta, or sends a characters, cycles, or items rate to another measure.
    #[test]
    fn adapter_rust_divan_binary() {
        let results = convert_v1(Adapter::RustDivan, "rust/divan/bytes_binary_shapes.txt");
        assert_reading(&results, &binary_shapes(), skipped(0), &[]);
        let results = convert_v1(Adapter::RustDivan, "rust/divan/bytes_binary_alloc.txt");
        assert_reading(&results, &binary_alloc(), skipped(0), &[ALLOCATED_UNIT]);
    }

    /// An `inf` rate is a time of zero, so it is skipped beside its finite columns, uncounted.
    ///
    /// Kills a reader that keeps an infinite rate, drops the row's finite rates with it, or counts
    /// it.
    #[test]
    fn adapter_rust_divan_zero_time() {
        let results = convert_v1(Adapter::RustDivan, "rust/divan/zero_time_inf.txt");
        assert_reading(&results, &zero_time(), skipped(0), &[]);
    }

    /// Kills a reader that loses the benchmarks before a panic, or does not count its unfinished
    /// row.
    #[test]
    fn adapter_rust_divan_panic() {
        let results = convert_v1(Adapter::RustDivan, "rust/divan/fail_default.txt");
        assert_reading(&results, &fail(), skipped(1), &[]);
    }

    /// With `--no-fail-fast`, the next target's header follows a panic's unfinished row on its line.
    ///
    /// Kills a reader that misses that header, names its tree after the row, or does not count the
    /// row.
    #[test]
    fn adapter_rust_divan_panic_then_target() {
        let results = convert_v1(Adapter::RustDivan, "rust/divan/fail_no_fail_fast.txt");
        assert_reading(&results, &fail_no_fail_fast(), skipped(1), &[]);
    }

    /// Kills a reader that refuses or misreads minutes.
    #[test]
    fn adapter_rust_divan_minutes() {
        let results = convert_v1(Adapter::RustDivan, "rust/divan/minute_default.txt");
        assert_reading(
            &results,
            &[("minute::one_minute", vec![latency([1.016 * MIN; 4])])],
            skipped(0),
            &[],
        );
    }

    /// A library with no tests prints libtest's plural preamble, `running 0 tests`, before every
    /// tree; the run's only skips are the edge tree's 4 and the panic's unfinished row.
    ///
    /// Kills a reader that counts the plural preamble.
    #[test]
    fn adapter_rust_divan_no_tests_preamble() {
        let results = convert_v1(Adapter::RustDivan, "rust/divan/all_targets.txt");
        assert_reading(&results, &all_targets(), skipped(5), &[ALLOCATED_UNIT]);
    }

    /// libtest names the reason of a test ignored with one, `test tests::slow ... ignored, too slow`.
    ///
    /// Kills a reader that counts an ignored test's line when it gives a reason.
    #[test]
    fn adapter_rust_divan_ignored_with_a_reason() {
        let results = convert_v1(Adapter::RustDivan, "rust/divan/lib_ignored_reason.txt");
        assert_reading(
            &results,
            &[
                ("ovf::a::t=1", vec![latency([0.0, 0.793, 0.001, 0.024])]),
                ("ovf::a::t=16", vec![latency([0.002, 0.233, 0.099, 0.083])]),
            ],
            skipped(0),
            &[],
        );
    }

    /// A nightly `cargo bench` of a libtest `#[bench]` and a Divan target prints both, and `magic`
    /// reads it with libtest's text leaf, which it tries first.
    ///
    /// Kills: a `magic` that tries Divan before libtest.
    #[test]
    fn adapter_rust_divan_after_libtest() {
        let results = convert_v1(Adapter::Magic, "rust/bench/divan_mixed.txt");
        assert_eq!(results.resolved_adapter, Some(Adapter::RustBenchText));
        assert_reading(
            &results,
            &[(
                "benches::bench_add",
                vec![(Latency::SLUG_STR, vec![("median", 0.30), ("range", 0.02)])],
            )],
            Skipped::default(),
            &[],
        );
        // Divan's leaf claims the run too, counting the libtest bench line, so the order decides.
        let divan = convert_v1(Adapter::RustDivan, "rust/bench/divan_mixed.txt");
        assert_reading(
            &divan,
            &[
                ("ovf::a::t=1", vec![latency([0.002, 1.477, 0.002, 0.02])]),
                ("ovf::a::t=16", vec![latency([0.004, 1.412, 0.238, 0.202])]),
            ],
            skipped(1),
            &[],
        );
    }

    /// Kills a reader that counts libtest's lines for a library's tests.
    #[test]
    fn adapter_rust_divan_libtest_preamble() {
        let results = convert_v1(Adapter::RustDivan, "rust/divan/lib_with_test.txt");
        assert_reading(&results, &lib_with_test(), skipped(0), &[]);
    }

    /// Kills a reader that reads one tree of several, carries a tree's path into the next, makes
    /// an ignored row a benchmark or a count, or reads seconds wrong.
    #[test]
    fn adapter_rust_divan_targets() {
        let results = convert_v1(Adapter::RustDivan, "rust/divan/multi_targets.txt");
        assert_reading(&results, &multi_targets(), skipped(0), &[ALLOCATED_UNIT]);
    }

    /// A workspace's `cargo bench` prints each crate's libtest lines after the tree before it.
    ///
    /// Kills a reader that keeps a tree open past its blank line, which counts the next crate's
    /// libtest lines.
    #[test]
    fn adapter_rust_divan_workspace() {
        let results = convert_v1(Adapter::RustDivan, "rust/divan/workspace.txt");
        assert_reading(
            &results,
            &[
                (
                    "first_bench::add",
                    vec![latency([0.002, 0.503, 0.002, 0.009])],
                ),
                (
                    "second_bench::add",
                    vec![latency([0.1, 0.822, 0.101, 0.156])],
                ),
            ],
            skipped(0),
            &[],
        );
    }

    /// An argument with a newline breaks its row, and the rate row under it then follows its
    /// sibling's prefix.
    ///
    /// Kills a reader that reads that rate as the sibling's, overwriting it.
    #[test]
    fn adapter_rust_divan_newline_beside_a_sibling() {
        let results = convert_v1(Adapter::RustDivan, "rust/divan/newline_counter.txt");
        assert_reading(
            &results,
            &[
                (
                    "args::counted::first",
                    vec![
                        latency([0.161, 1.187, 0.163, 0.175]),
                        row(
                            BYTES_RATE,
                            [31.05 * GIGA, 4.212 * GIGA, 30.67 * GIGA, 28.57 * GIGA],
                        ),
                    ],
                ),
                (
                    "args::counted::next",
                    vec![
                        latency([0.16, 1.118, 0.162, 0.172]),
                        row(
                            BYTES_RATE,
                            [25.0 * GIGA, 3.577 * GIGA, 24.69 * GIGA, 23.25 * GIGA],
                        ),
                    ],
                ),
            ],
            skipped(3),
            &[],
        );
    }

    /// Two crates of a workspace with a bench target of one name print the same paths.
    ///
    /// Kills a reader that lets the second overwrite the first, uncounted.
    #[test]
    fn adapter_rust_divan_same_target() {
        let results = convert_v1(Adapter::RustDivan, "rust/divan/same_target.txt");
        assert_reading(
            &results,
            &[("bench::add", vec![latency([0.098, 0.503, 0.099, 0.103])])],
            Skipped {
                variants: 1,
                ..Skipped::default()
            },
            &[],
        );
    }

    /// Issue 321: a generic type and an argument stay in the name.
    #[test]
    fn adapter_rust_divan_issue_321() {
        let results = convert_v1(Adapter::RustDivan, "rust/divan/issue_321.txt");
        assert_reading(&results, &issue_321(), skipped(0), &[]);
    }

    /// A `cargo bench` of criterion and Divan prints both: `magic` reads the Divan tree and counts
    /// criterion's line.
    ///
    /// Kills: a `magic` that tries criterion's text leaf before Divan, which drops the tree.
    #[test]
    fn adapter_rust_divan_before_criterion() {
        let results = convert_v1(Adapter::Magic, "rust/divan/mixed_crit_divan.txt");
        assert_eq!(results.resolved_adapter, Some(Adapter::RustDivan));
        assert_reading(&results, &mixed(), skipped(1), &[]);
        // criterion's text leaf claims the run too, as its one benchmark, so the order decides.
        let criterion = convert_v1(
            Adapter::RustCriterionText,
            "rust/divan/mixed_crit_divan.txt",
        );
        assert_eq!(criterion.inner.len(), 1);
    }

    /// Divan prints hours past 59.99 minutes and days past 23.99 hours, and whole days from 10,000
    /// days; the values are synthetic, in Divan's own layout.
    ///
    /// Kills a reader that refuses or misreads `h` or `d`.
    #[test]
    fn adapter_rust_divan_long_times() {
        let results = AdapterRustDivan::parse(
            "long          fastest       │ slowest       │ median        │ mean          │ samples │ iters\n\
             ├─ hours      1.5 h         │ 2 h           │ 1.75 h        │ 1.75 h        │ 2       │ 2\n\
             ╰─ days       1.25 d        │ 10000 d       │ 5000 d        │ 5000 d        │ 2       │ 2\n\n",
            Settings::default(),
        )
        .expect("Failed to parse");
        assert_reading(
            &results,
            &[
                (
                    "long::hours",
                    vec![latency([1.5 * HOUR, 2.0 * HOUR, 1.75 * HOUR, 1.75 * HOUR])],
                ),
                (
                    "long::days",
                    vec![latency([
                        1.25 * DAY,
                        10_000.0 * DAY,
                        5_000.0 * DAY,
                        5_000.0 * DAY,
                    ])],
                ),
            ],
            skipped(0),
            &[],
        );
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_rust_divan_ignores_average() {
        for suffix in [
            "rust/divan/alloc_default.txt",
            "rust/divan/zero_time_inf.txt",
        ] {
            assert_ignores_average(Adapter::RustDivan, suffix);
        }
    }

    /// A benchmark's name and every measure expected of it.
    type Reading = (&'static str, Vec<Expected>);

    /// The results hold exactly these benchmarks, each with one variant of exactly its expected
    /// measures, these skips, and these measures' units.
    fn assert_reading(
        results: &AdapterResults,
        readings: &[Reading],
        skipped: Skipped,
        units: &[(&str, &str)],
    ) {
        let names: BTreeSet<String> = results.inner.keys().map(ToString::to_string).collect();
        let expected: BTreeSet<String> = readings
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .collect();
        assert_eq!(names, expected);
        assert_eq!(variants(results), readings.len());
        for (benchmark, measures) in readings {
            assert_variant(results, benchmark, "{}", measures);
        }
        assert_eq!(results.skipped, skipped);
        assert_units(results, units);
    }

    fn skipped(benchmarks: usize) -> Skipped {
        Skipped {
            benchmarks,
            ..Skipped::default()
        }
    }

    /// A benchmark's times: fastest, slowest, median, and mean.
    fn latency(times: [f64; 4]) -> Expected {
        (
            Latency::SLUG_STR,
            ["min", "max", "median", "mean"]
                .into_iter()
                .zip(times)
                .collect(),
        )
    }

    /// A row under a benchmark's times, under Divan's column names, without its infinite values.
    fn row(measure: &'static str, values: [f64; 4]) -> Expected {
        (
            measure,
            ["fastest", "slowest", "median", "mean"]
                .into_iter()
                .zip(values)
                .filter(|(_, value)| value.is_finite())
                .collect(),
        )
    }

    const ALLOCATED_UNIT: (&str, &str) = (ALLOCATED, Allocated::UNITS_STR);

    const BYTES_RATE: &str = BytesThroughput::SLUG_STR;
    const CHARS_RATE: &str = CharsThroughput::SLUG_STR;
    const CYCLES_RATE: &str = CyclesThroughput::SLUG_STR;
    const ITEMS_RATE: &str = Throughput::SLUG_STR;
    const MAX_ALLOCATIONS: &str = MaxAllocations::SLUG_STR;
    const MAX_ALLOCATED: &str = MaxAllocated::SLUG_STR;
    const ALLOCATIONS: &str = Allocations::SLUG_STR;
    const ALLOCATED: &str = Allocated::SLUG_STR;
    const DEALLOCATIONS: &str = Deallocations::SLUG_STR;
    const DEALLOCATED: &str = Deallocated::SLUG_STR;
    const GROWS: &str = Grows::SLUG_STR;
    const GROWN: &str = Grown::SLUG_STR;
    const SHRINKS: &str = Shrinks::SLUG_STR;
    const SHRUNK: &str = Shrunk::SLUG_STR;

    const INF: f64 = f64::INFINITY;
    const US: f64 = 1e3;
    const MS: f64 = 1e6;
    const S: f64 = 1e9;
    const MIN: f64 = 60.0 * S;
    const HOUR: f64 = 60.0 * MIN;
    const DAY: f64 = 24.0 * HOUR;
    const KILO: f64 = 1e3;
    const MEGA: f64 = 1e6;
    const GIGA: f64 = 1e9;
    const TERA: f64 = 1e12;
    const PETA: f64 = 1e15;
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * KIB;
    const GIB: f64 = 1024.0 * MIB;
    const PIB: f64 = 1024.0 * 1024.0 * GIB;

    /// `edge_default.txt`: 5 benchmarks, 4 skipped.
    fn edge() -> Vec<Reading> {
        vec![
            ("edge::t=4", vec![latency([0.001; 4])]),
            (
                "edge::weird_args::   padded",
                vec![latency([0.263, 0.269, 0.269, 0.268])],
            ),
            (
                "edge::weird_args::fastest │ slowest",
                vec![latency([0.263, 0.273, 0.269, 0.268])],
            ),
            (
                "edge::weird_args::t=1",
                vec![latency([0.312, 1.745, 0.317, 0.332])],
            ),
            ("edge::├─ fake branch", vec![latency([0.001; 4])]),
        ]
    }

    /// `ovf_default.txt`: 2 benchmarks, 0 skipped.
    fn ovf() -> Vec<Reading> {
        vec![
            ("ovf::a::t=1", vec![latency([0.001; 4])]),
            ("ovf::a::t=16", vec![latency([0.004, 0.295, 0.134, 0.135])]),
        ]
    }

    /// `alloc_default.txt`: 10 benchmarks, 0 skipped.
    #[expect(clippy::too_many_lines, reason = "every benchmark of a real capture")]
    fn alloc() -> Vec<Reading> {
        vec![
            (
                "alloc::big_alloc",
                vec![
                    latency([52.22 * US, 2.171 * MS, 52.29 * US, 88.31 * US]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [8.388 * MEGA; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [8.388 * MEGA; 4]),
                ],
            ),
            (
                "alloc::box_alloc",
                vec![
                    latency([7.498, 8.557, 7.854, 7.831]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [8.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [8.0; 4]),
                ],
            ),
            (
                "alloc::no_alloc",
                vec![latency([0.05, 0.053, 0.052, 0.052])],
            ),
            (
                "alloc::string_format",
                vec![
                    latency([31.24, 36.86, 31.72, 31.84]),
                    row(
                        BYTES_RATE,
                        [352.0 * MEGA, 298.3 * MEGA, 346.7 * MEGA, 345.4 * MEGA],
                    ),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [16.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [8.0; 4]),
                    row(GROWS, [1.0; 4]),
                    row(GROWN, [8.0; 4]),
                ],
            ),
            (
                "alloc::threaded_alloc::t=1",
                vec![
                    latency([11.29, 37.23, 11.59, 11.81]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [256.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [256.0; 4]),
                ],
            ),
            (
                "alloc::threaded_alloc::t=4",
                vec![
                    latency([20.03, 131.4, 21.31, 29.95]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [256.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [256.0; 4]),
                ],
            ),
            (
                "alloc::vec_push_grow::1",
                vec![
                    latency([18.57, 19.82, 18.77, 18.81]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [32.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [32.0; 4]),
                ],
            ),
            (
                "alloc::vec_push_grow::100",
                vec![
                    latency([114.9, 464.8, 143.1, 145.0]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [1.024 * KILO; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [32.0; 4]),
                    row(GROWS, [5.0; 4]),
                    row(GROWN, [992.0; 4]),
                ],
            ),
            (
                "alloc::vec_push_grow::10000",
                vec![
                    latency([3.958 * US, 6.031 * US, 4.049 * US, 4.106 * US]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [131.0 * KILO; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [32.0; 4]),
                    row(GROWS, [12.0; 4]),
                    row(GROWN, [131.0 * KILO; 4]),
                ],
            ),
            (
                "alloc::vec_shrink",
                vec![
                    latency([29.52, 62.93, 29.6, 29.94]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [71.93; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [8.192 * KILO; 4]),
                    row(SHRINKS, [1.0; 4]),
                    row(SHRUNK, [8.184 * KILO; 4]),
                ],
            ),
        ]
    }

    /// `more_default.txt`: 16 benchmarks, 0 skipped.
    fn more() -> Vec<Reading> {
        vec![
            (
                "more::alloc_and_free",
                vec![
                    latency([5.732, 27.48, 5.81, 6.041]),
                    row(MAX_ALLOCATIONS, [0.003; 4]),
                    row(MAX_ALLOCATED, [0.093; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [24.0; 4]),
                    row(DEALLOCATIONS, [1.0; 4]),
                    row(DEALLOCATED, [24.0; 4]),
                ],
            ),
            (
                "more::counter_threads::t=1",
                vec![
                    latency([0.325, 5.162, 0.335, 0.409]),
                    row(
                        ITEMS_RATE,
                        [30.76 * GIGA, 1.937 * GIGA, 29.85 * GIGA, 24.44 * GIGA],
                    ),
                ],
            ),
            (
                "more::counter_threads::t=2",
                vec![
                    latency([0.325, 8.35, 0.391, 0.736]),
                    row(
                        ITEMS_RATE,
                        [30.76 * GIGA, 1.197 * GIGA, 25.57 * GIGA, 13.58 * GIGA],
                    ),
                ],
            ),
            (
                "more::with_inputs_values",
                vec![
                    latency([9.658, 10.21, 9.822, 9.8]),
                    row(DEALLOCATIONS, [1.0; 4]),
                    row(DEALLOCATED, [64.0; 4]),
                ],
            ),
            (
                "more::deep_combo::u8::1::3::t=1",
                vec![latency([0.015, 1.509, 0.071, 0.084])],
            ),
            (
                "more::deep_combo::u8::1::3::t=2",
                vec![latency([0.037, 0.166, 0.061, 0.062])],
            ),
            (
                "more::deep_combo::u8::1::4::t=1",
                vec![latency([0.068, 0.076, 0.073, 0.072])],
            ),
            (
                "more::deep_combo::u8::1::4::t=2",
                vec![latency([0.034, 1.235, 0.041, 0.06])],
            ),
            (
                "more::deep_combo::u8::2::3::t=1",
                vec![latency([0.031, 1.963, 0.037, 0.056])],
            ),
            (
                "more::deep_combo::u8::2::3::t=2",
                vec![latency([0.012, 0.064, 0.037, 0.038])],
            ),
            (
                "more::deep_combo::u8::2::4::t=1",
                vec![latency([0.01, 1.144, 0.034, 0.047])],
            ),
            (
                "more::deep_combo::u8::2::4::t=2",
                vec![latency([0.027, 0.528, 0.045, 0.058])],
            ),
            (
                "more::generic renamed::u8",
                vec![latency([0.001, 0.298, 0.001, 0.004])],
            ),
            ("more::generic renamed::u16", vec![latency([0.001; 4])]),
            ("more::threaded_group::inc::t=1", vec![latency([0.001; 4])]),
            (
                "more::threaded_group::inc::t=2",
                vec![latency([0.001, 0.341, 0.001, 0.004])],
            ),
        ]
    }

    /// `bytes_binary_shapes.txt`: 10 benchmarks, 0 skipped.
    #[expect(clippy::too_many_lines, reason = "every benchmark of a real capture")]
    #[expect(
        clippy::approx_constant,
        reason = "measured times whose digits match a constant"
    )]
    fn binary_shapes() -> Vec<Reading> {
        vec![
            (
                "shapes::counter_all_four",
                vec![
                    latency([8.567, 51.13, 9.149, 9.442]),
                    row(
                        BYTES_RATE,
                        [111.3 * GIB, 18.65 * GIB, 104.2 * GIB, 101.0 * GIB],
                    ),
                    row(
                        CHARS_RATE,
                        [1.167 * GIGA, 195.5 * MEGA, 1.093 * GIGA, 1.059 * GIGA],
                    ),
                    row(
                        CYCLES_RATE,
                        [116.7 * GIGA, 19.55 * GIGA, 109.3 * GIGA, 105.9 * GIGA],
                    ),
                    row(
                        ITEMS_RATE,
                        [1.867 * GIGA, 312.9 * MEGA, 1.748 * GIGA, 1.694 * GIGA],
                    ),
                ],
            ),
            (
                "shapes::counter_bytes",
                vec![
                    latency([9.11, 88.92, 9.192, 9.998]),
                    row(
                        BYTES_RATE,
                        [104.6 * GIB, 10.72 * GIB, 103.7 * GIB, 95.38 * GIB],
                    ),
                ],
            ),
            (
                "shapes::counter_bytes_args::64",
                vec![
                    latency([6.235, 6.28, 6.257, 6.262]),
                    row(
                        BYTES_RATE,
                        [9.559 * GIB, 9.491 * GIB, 9.526 * GIB, 9.518 * GIB],
                    ),
                ],
            ),
            (
                "shapes::counter_bytes_args::4096",
                vec![
                    latency([406.5, 1.677 * US, 407.9, 429.7]),
                    row(
                        BYTES_RATE,
                        [9.383 * GIB, 2.274 * GIB, 9.351 * GIB, 8.876 * GIB],
                    ),
                ],
            ),
            (
                "shapes::counter_bytes_args::1048576",
                vec![
                    latency([103.4 * US, 119.3 * US, 103.5 * US, 105.1 * US]),
                    row(
                        BYTES_RATE,
                        [9.438 * GIB, 8.184 * GIB, 9.429 * GIB, 9.287 * GIB],
                    ),
                ],
            ),
            (
                "shapes::counter_chars_of_str",
                vec![
                    latency([2.579, 2.599, 2.589, 2.589]),
                    row(
                        CHARS_RATE,
                        [4.265 * GIGA, 4.232 * GIGA, 4.248 * GIGA, 4.248 * GIGA],
                    ),
                ],
            ),
            (
                "shapes::counter_huge_bytes",
                vec![
                    latency([0.003, 0.009, 0.006, 0.006]),
                    row(
                        BYTES_RATE,
                        [
                            325_520_833.0 * PIB,
                            108_506_944.0 * PIB,
                            162_760_416.0 * PIB,
                            162_760_416.0 * PIB,
                        ],
                    ),
                ],
            ),
            (
                "shapes::counter_per_input",
                vec![
                    latency([34.2, 769.1, 371.1, 390.6]),
                    row(
                        BYTES_RATE,
                        [4.002 * GIB, 4.789 * GIB, 4.745 * GIB, 4.705 * GIB],
                    ),
                ],
            ),
            (
                "shapes::counter_runtime_items",
                vec![
                    latency([0.437, 0.449, 0.442, 0.443]),
                    row(
                        ITEMS_RATE,
                        [2.288 * TERA, 2.227 * TERA, 2.262 * TERA, 2.257 * TERA],
                    ),
                ],
            ),
            (
                "shapes::counter_zero_items",
                vec![
                    latency([0.001, 0.329, 0.007, 0.016]),
                    row(ITEMS_RATE, [0.0; 4]),
                ],
            ),
        ]
    }

    /// `bytes_binary_alloc.txt`: 10 benchmarks, 0 skipped.
    #[expect(clippy::too_many_lines, reason = "every benchmark of a real capture")]
    fn binary_alloc() -> Vec<Reading> {
        vec![
            (
                "alloc::big_alloc",
                vec![
                    latency([52.41 * US, 1.784 * MS, 52.61 * US, 84.94 * US]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [8.0 * MIB; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [8.0 * MIB; 4]),
                ],
            ),
            (
                "alloc::box_alloc",
                vec![
                    latency([7.581, 9.933, 8.089, 8.061]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [8.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [8.0; 4]),
                ],
            ),
            (
                "alloc::no_alloc",
                vec![latency([0.051, 0.054, 0.053, 0.052])],
            ),
            (
                "alloc::string_format",
                vec![
                    latency([30.46, 88.84, 31.56, 32.02]),
                    row(
                        BYTES_RATE,
                        [344.3 * MIB, 118.0 * MIB, 332.3 * MIB, 327.5 * MIB],
                    ),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [16.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [8.0; 4]),
                    row(GROWS, [1.0; 4]),
                    row(GROWN, [8.0; 4]),
                ],
            ),
            (
                "alloc::threaded_alloc::t=1",
                vec![
                    latency([11.33, 12.03, 11.67, 11.69]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [256.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [256.0; 4]),
                ],
            ),
            (
                "alloc::threaded_alloc::t=4",
                vec![
                    latency([29.4, 1.12 * US, 39.65, 73.66]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [256.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [256.0; 4]),
                ],
            ),
            (
                "alloc::vec_push_grow::1",
                vec![
                    latency([17.08, 19.67, 17.32, 17.43]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [32.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [32.0; 4]),
                ],
            ),
            (
                "alloc::vec_push_grow::100",
                vec![
                    latency([124.9, 186.9, 134.6, 137.7]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [1.0 * KIB; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [32.0; 4]),
                    row(GROWS, [5.0; 4]),
                    row(GROWN, [992.0; 4]),
                ],
            ),
            (
                "alloc::vec_push_grow::10000",
                vec![
                    latency([3.658 * US, 11.48 * US, 3.718 * US, 4.052 * US]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [128.0 * KIB; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [32.0; 4]),
                    row(GROWS, [12.0; 4]),
                    row(GROWN, [127.9 * KIB; 4]),
                ],
            ),
            (
                "alloc::vec_shrink",
                vec![
                    latency([27.33, 27.65, 27.41, 27.42]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [71.93; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [8.0 * KIB; 4]),
                    row(SHRINKS, [1.0; 4]),
                    row(SHRUNK, [7.992 * KIB; 4]),
                ],
            ),
        ]
    }

    /// `zero_time_inf.txt`: 12 benchmarks, 0 skipped.
    #[expect(clippy::too_many_lines, reason = "every benchmark of a real capture")]
    fn zero_time() -> Vec<Reading> {
        vec![
            (
                "shapes::noop",
                vec![
                    latency([0.002, 0.073, 0.003, 0.006]),
                    row(
                        ITEMS_RATE,
                        [50.0 * TERA, 1.369 * TERA, 33.33 * TERA, 16.66 * TERA],
                    ),
                ],
            ),
            (
                "shapes::renamed with spaces",
                vec![
                    latency([0.001, 0.083, 0.005, 0.007]),
                    row(
                        ITEMS_RATE,
                        [100.0 * TERA, 1.204 * TERA, 20.0 * TERA, 14.28 * TERA],
                    ),
                ],
            ),
            (
                "shapes::bool_consts::false",
                vec![
                    latency([0.0, 0.146, 0.012, 0.018]),
                    row(ITEMS_RATE, [INF, 684.9 * GIGA, 8.333 * TERA, 5.555 * TERA]),
                ],
            ),
            (
                "shapes::bool_consts::true",
                vec![
                    latency([0.0, 0.001, 0.001, 0.001]),
                    row(ITEMS_RATE, [INF, 100.0 * TERA, 100.0 * TERA, 100.0 * TERA]),
                ],
            ),
            (
                "shapes::generic_consts::1",
                vec![
                    latency([0.0, 0.492, 0.001, 0.005]),
                    row(ITEMS_RATE, [INF, 203.2 * GIGA, 100.0 * TERA, 20.0 * TERA]),
                ],
            ),
            (
                "shapes::generic_consts::8",
                vec![
                    latency([0.001, 0.387, 0.001, 0.005]),
                    row(
                        ITEMS_RATE,
                        [100.0 * TERA, 258.3 * GIGA, 100.0 * TERA, 20.0 * TERA],
                    ),
                ],
            ),
            (
                "shapes::generic_consts::64",
                vec![
                    latency([0.55, 1.982, 0.552, 0.567]),
                    row(
                        ITEMS_RATE,
                        [181.8 * GIGA, 50.45 * GIGA, 181.1 * GIGA, 176.3 * GIGA],
                    ),
                ],
            ),
            (
                "shapes::generic_consts::1000",
                vec![
                    latency([8.641, 8.919, 8.762, 8.777]),
                    row(
                        ITEMS_RATE,
                        [11.57 * GIGA, 11.21 * GIGA, 11.41 * GIGA, 11.39 * GIGA],
                    ),
                ],
            ),
            (
                "shapes::threads_types::u8::t=1",
                vec![latency([0.001; 4]), row(ITEMS_RATE, [100.0 * TERA; 4])],
            ),
            (
                "shapes::threads_types::u8::t=2",
                vec![
                    latency([0.002, 0.171, 0.097, 0.066]),
                    row(
                        ITEMS_RATE,
                        [50.0 * TERA, 584.7 * GIGA, 1.03 * TERA, 1.515 * TERA],
                    ),
                ],
            ),
            (
                "shapes::threads_types::u64::t=1",
                vec![
                    latency([0.001, 0.452, 0.001, 0.007]),
                    row(
                        ITEMS_RATE,
                        [100.0 * TERA, 221.2 * GIGA, 100.0 * TERA, 14.28 * TERA],
                    ),
                ],
            ),
            (
                "shapes::threads_types::u64::t=2",
                vec![
                    latency([0.141, 0.176, 0.155, 0.154]),
                    row(
                        ITEMS_RATE,
                        [709.2 * GIGA, 568.1 * GIGA, 645.1 * GIGA, 649.3 * GIGA],
                    ),
                ],
            ),
        ]
    }

    /// `fail_default.txt`: 1 benchmarks, 1 skipped.
    fn fail() -> Vec<Reading> {
        vec![("fail::a_before", vec![latency([0.016, 0.021, 0.02, 0.02])])]
    }

    /// `lib_with_test.txt`: 2 benchmarks, 0 skipped.
    fn lib_with_test() -> Vec<Reading> {
        vec![
            ("ovf::a::t=1", vec![latency([0.001, 0.337, 0.001, 0.006])]),
            ("ovf::a::t=16", vec![latency([0.001, 0.179, 0.005, 0.053])]),
        ]
    }

    /// `mixed_crit_divan.txt`: 2 benchmarks, 1 skipped.
    fn mixed() -> Vec<Reading> {
        vec![
            ("ovf::a::t=1", vec![latency([0.001, 0.006, 0.002, 0.002])]),
            ("ovf::a::t=16", vec![latency([0.002, 0.812, 0.006, 0.025])]),
        ]
    }

    /// `issue_321.txt`: 5 benchmarks, 0 skipped.
    fn issue_321() -> Vec<Reading> {
        vec![
            (
                "scratch::from_str::&str",
                vec![latency([0.738, 0.799, 0.759, 0.757])],
            ),
            (
                "scratch::from_str::String",
                vec![latency([26.8, 32.18, 30.39, 30.42])],
            ),
            (
                "scratch::init_array::1000",
                vec![latency([572.5, 598.6, 583.0, 584.5])],
            ),
            (
                "scratch::init_array::2000",
                vec![latency([1.155 * US, 1.197 * US, 1.166 * US, 1.165 * US])],
            ),
            (
                "scratch::init_array::3000",
                vec![latency([1.759 * US, 1.801 * US, 1.77 * US, 1.77 * US])],
            ),
        ]
    }

    /// `multi_targets.txt`: 88 benchmarks, 0 skipped.
    #[expect(clippy::too_many_lines, reason = "every benchmark of a real capture")]
    #[expect(
        clippy::approx_constant,
        reason = "measured times whose digits match a constant"
    )]
    fn multi_targets() -> Vec<Reading> {
        vec![
            (
                "alloc::big_alloc",
                vec![
                    latency([48.52 * US, 1.78 * MS, 48.89 * US, 81.77 * US]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [8.388 * MEGA; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [8.388 * MEGA; 4]),
                ],
            ),
            (
                "alloc::box_alloc",
                vec![
                    latency([3.198, 12.44, 3.283, 3.673]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [8.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [8.0; 4]),
                ],
            ),
            (
                "alloc::no_alloc",
                vec![latency([0.003, 0.237, 0.01, 0.012])],
            ),
            (
                "alloc::string_format",
                vec![
                    latency([28.84, 105.6, 29.16, 30.38]),
                    row(
                        BYTES_RATE,
                        [381.3 * MEGA, 104.0 * MEGA, 377.1 * MEGA, 361.9 * MEGA],
                    ),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [16.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [8.0; 4]),
                    row(GROWS, [1.0; 4]),
                    row(GROWN, [8.0; 4]),
                ],
            ),
            (
                "alloc::threaded_alloc::t=1",
                vec![
                    latency([11.02, 48.35, 11.26, 12.08]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [256.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [256.0; 4]),
                ],
            ),
            (
                "alloc::threaded_alloc::t=4",
                vec![
                    latency([24.44, 4.025 * US, 39.44, 121.0]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [256.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [256.0; 4]),
                ],
            ),
            (
                "alloc::vec_push_grow::1",
                vec![
                    latency([16.33, 51.24, 16.8, 17.16]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [32.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [32.0; 4]),
                ],
            ),
            (
                "alloc::vec_push_grow::100",
                vec![
                    latency([120.9, 977.1, 182.2, 186.8]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [1.024 * KILO; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [32.0; 4]),
                    row(GROWS, [5.0; 4]),
                    row(GROWN, [992.0; 4]),
                ],
            ),
            (
                "alloc::vec_push_grow::10000",
                vec![
                    latency([3.608 * US, 9.287 * US, 3.699 * US, 3.806 * US]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [131.0 * KILO; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [32.0; 4]),
                    row(GROWS, [12.0; 4]),
                    row(GROWN, [131.0 * KILO; 4]),
                ],
            ),
            (
                "alloc::vec_shrink",
                vec![
                    latency([28.05, 45.58, 30.3, 35.78]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [263.7; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [8.192 * KILO; 4]),
                    row(SHRINKS, [1.0; 4]),
                    row(SHRUNK, [8.184 * KILO; 4]),
                ],
            ),
            ("ovf::a::t=1", vec![latency([0.001, 0.3, 0.007, 0.009])]),
            ("ovf::a::t=16", vec![latency([0.049, 2.44, 0.242, 0.234])]),
            (
                "shapes::args_debug::None",
                vec![latency([0.187, 0.74, 0.188, 0.195])],
            ),
            (
                "shapes::args_debug::Some(1)",
                vec![latency([0.318, 0.526, 0.32, 0.325])],
            ),
            (
                "shapes::args_floats::1.5",
                vec![latency([0.241, 0.311, 0.242, 0.245])],
            ),
            (
                "shapes::args_floats::2.25",
                vec![latency([0.241, 0.298, 0.242, 0.244])],
            ),
            (
                "shapes::args_ints::1",
                vec![latency([0.721, 1.714, 0.724, 0.738])],
            ),
            (
                "shapes::args_ints::10",
                vec![latency([0.721, 1.247, 0.724, 0.739])],
            ),
            (
                "shapes::args_ints::100",
                vec![latency([0.721, 1.946, 0.724, 0.745])],
            ),
            (
                "shapes::args_of_ranges::0..10",
                vec![latency([0.763, 0.978, 0.765, 0.77])],
            ),
            (
                "shapes::args_of_ranges::5..500",
                vec![latency([0.821, 1.305, 0.831, 0.841])],
            ),
            (
                "shapes::args_range::1",
                vec![latency([0.199, 0.274, 0.201, 0.202])],
            ),
            (
                "shapes::args_range::2",
                vec![latency([0.199, 0.861, 0.201, 0.214])],
            ),
            (
                "shapes::args_range::3",
                vec![latency([0.199, 4.745, 0.201, 0.248])],
            ),
            (
                "shapes::args_strs::",
                vec![latency([0.248, 0.667, 0.249, 0.255])],
            ),
            (
                "shapes::args_strs::a │ b",
                vec![latency([0.248, 0.943, 0.249, 0.261])],
            ),
            (
                "shapes::args_strs::hello",
                vec![latency([0.247, 2.56, 0.249, 0.274])],
            ),
            (
                "shapes::args_strs::hello world",
                vec![latency([0.254, 0.838, 0.257, 0.267])],
            ),
            (
                "shapes::args_strs::ünïcödé",
                vec![latency([0.248, 0.674, 0.251, 0.256])],
            ),
            (
                "shapes::counter_all_four",
                vec![
                    latency([8.327, 20.25, 8.366, 8.516]),
                    row(
                        BYTES_RATE,
                        [122.9 * GIGA, 50.55 * GIGA, 122.4 * GIGA, 120.2 * GIGA],
                    ),
                    row(
                        CHARS_RATE,
                        [1.2 * GIGA, 493.6 * MEGA, 1.195 * GIGA, 1.174 * GIGA],
                    ),
                    row(
                        CYCLES_RATE,
                        [120.0 * GIGA, 49.36 * GIGA, 119.5 * GIGA, 117.4 * GIGA],
                    ),
                    row(
                        ITEMS_RATE,
                        [1.921 * GIGA, 789.8 * MEGA, 1.912 * GIGA, 1.878 * GIGA],
                    ),
                ],
            ),
            (
                "shapes::counter_bytes",
                vec![
                    latency([8.327, 11.57, 8.37, 8.404]),
                    row(
                        BYTES_RATE,
                        [122.9 * GIGA, 88.48 * GIGA, 122.3 * GIGA, 121.8 * GIGA],
                    ),
                ],
            ),
            (
                "shapes::counter_bytes_args::64",
                vec![
                    latency([5.823, 15.19, 5.844, 5.984]),
                    row(
                        BYTES_RATE,
                        [10.99 * GIGA, 4.212 * GIGA, 10.95 * GIGA, 10.69 * GIGA],
                    ),
                ],
            ),
            (
                "shapes::counter_bytes_args::4096",
                vec![
                    latency([380.3, 428.1, 381.6, 382.2]),
                    row(
                        BYTES_RATE,
                        [10.77 * GIGA, 9.566 * GIGA, 10.73 * GIGA, 10.71 * GIGA],
                    ),
                ],
            ),
            (
                "shapes::counter_bytes_args::1048576",
                vec![
                    latency([96.42 * US, 103.4 * US, 96.86 * US, 97.73 * US]),
                    row(
                        BYTES_RATE,
                        [10.87 * GIGA, 10.13 * GIGA, 10.82 * GIGA, 10.72 * GIGA],
                    ),
                ],
            ),
            (
                "shapes::counter_chars_of_str",
                vec![
                    latency([2.39, 3.79, 2.41, 2.432]),
                    row(
                        CHARS_RATE,
                        [4.602 * GIGA, 2.902 * GIGA, 4.564 * GIGA, 4.523 * GIGA],
                    ),
                ],
            ),
            (
                "shapes::counter_huge_bytes",
                vec![
                    latency([0.001, 0.419, 0.001, 0.007]),
                    row(
                        BYTES_RATE,
                        [
                            1_099_511_627.0 * PETA,
                            2_624_132.0 * PETA,
                            1_099_511_627.0 * PETA,
                            157_073_089.0 * PETA,
                        ],
                    ),
                ],
            ),
            (
                "shapes::counter_per_input",
                vec![
                    latency([32.33, 715.9, 349.0, 366.5]),
                    row(
                        BYTES_RATE,
                        [4.546 * GIGA, 5.524 * GIGA, 5.325 * GIGA, 5.385 * GIGA],
                    ),
                ],
            ),
            (
                "shapes::counter_runtime_items",
                vec![
                    latency([0.391, 0.567, 0.398, 0.4]),
                    row(
                        ITEMS_RATE,
                        [2.557 * TERA, 1.763 * TERA, 2.512 * TERA, 2.5 * TERA],
                    ),
                ],
            ),
            (
                "shapes::counter_zero_items",
                vec![
                    latency([0.001, 0.234, 0.001, 0.006]),
                    row(ITEMS_RATE, [0.0; 4]),
                ],
            ),
            ("shapes::noop", vec![latency([0.001, 0.007, 0.001, 0.001])]),
            (
                "shapes::plain_add",
                vec![latency([0.169, 0.57, 0.169, 0.174])],
            ),
            (
                "shapes::renamed with spaces",
                vec![latency([0.001, 0.155, 0.001, 0.002])],
            ),
            ("shapes::slow_sleep", vec![latency([1.5 * S; 4])]),
            (
                "shapes::threads_args::10::t=1",
                vec![latency([3.673, 24.18, 3.683, 3.917])],
            ),
            (
                "shapes::threads_args::10::t=2",
                vec![latency([4.083, 8.954, 7.096, 7.151])],
            ),
            (
                "shapes::threads_args::20::t=1",
                vec![latency([3.673, 8.934, 3.682, 3.737])],
            ),
            (
                "shapes::threads_args::20::t=2",
                vec![latency([5.883, 8.544, 6.753, 6.803])],
            ),
            (
                "shapes::threads_available",
                vec![latency([3.87, 57.49, 4.725, 12.98])],
            ),
            (
                "shapes::threads_list::t=1",
                vec![latency([3.514, 4.219, 3.966, 3.859])],
            ),
            (
                "shapes::threads_list::t=2",
                vec![latency([7.485, 13.33, 7.957, 8.098])],
            ),
            (
                "shapes::threads_list::t=4",
                vec![latency([11.61, 78.12, 17.48, 19.52])],
            ),
            (
                "shapes::threads_single_four",
                vec![latency([15.21, 24.44, 17.28, 18.58])],
            ),
            (
                "shapes::threads_zero_one::t=1",
                vec![latency([3.514, 4.182, 3.516, 3.642])],
            ),
            (
                "shapes::threads_zero_one::t=16",
                vec![latency([3.948, 23.75, 5.12, 7.538])],
            ),
            (
                "shapes::Named Group::x",
                vec![latency([0.002, 0.147, 0.002, 0.01])],
            ),
            ("shapes::bool_consts::false", vec![latency([0.001; 4])]),
            ("shapes::bool_consts::true", vec![latency([0.001; 4])]),
            (
                "shapes::generic_args::BTreeSet<u32>::4",
                vec![latency([19.51, 66.78, 19.83, 20.46])],
            ),
            (
                "shapes::generic_args::BTreeSet<u32>::64",
                vec![latency([124.3, 171.9, 126.2, 127.3])],
            ),
            (
                "shapes::generic_args::Vec<u32>::4",
                vec![latency([12.04, 12.51, 12.04, 12.06])],
            ),
            (
                "shapes::generic_args::Vec<u32>::64",
                vec![latency([13.33, 37.44, 13.41, 13.72])],
            ),
            (
                "shapes::generic_args::VecDeque<u32>::4",
                vec![latency([12.04, 12.28, 12.08, 12.07])],
            ),
            (
                "shapes::generic_args::VecDeque<u32>::64",
                vec![latency([13.56, 15.45, 13.64, 13.66])],
            ),
            (
                "shapes::generic_consts::1",
                vec![latency([0.001, 0.173, 0.001, 0.002])],
            ),
            ("shapes::generic_consts::8", vec![latency([0.001; 4])]),
            (
                "shapes::generic_consts::64",
                vec![latency([0.545, 0.55, 0.548, 0.548])],
            ),
            (
                "shapes::generic_consts::1000",
                vec![latency([8.561, 33.37, 8.76, 8.997])],
            ),
            (
                "shapes::generic_types::&str",
                vec![latency([0.001, 0.213, 0.001, 0.003])],
            ),
            (
                "shapes::generic_types::HashMap<u8, u8>",
                vec![latency([2.39, 2.402, 2.392, 2.395])],
            ),
            (
                "shapes::generic_types::String",
                vec![latency([0.078, 2.088, 0.08, 0.106])],
            ),
            (
                "shapes::generic_types::Vec<alloc::string::String>",
                vec![latency([0.078, 0.081, 0.079, 0.079])],
            ),
            ("shapes::generic_types::u8", vec![latency([0.001; 4])]),
            (
                "shapes::generic_types::u64",
                vec![latency([0.001, 0.196, 0.001, 0.003])],
            ),
            (
                "shapes::group_opts::a",
                vec![latency([1.8, 3.8, 1.8, 1.882])],
            ),
            (
                "shapes::group_opts::b",
                vec![latency([1.8, 3.8, 1.8, 2.385])],
            ),
            (
                "shapes::outer::shallow",
                vec![latency([0.001, 0.117, 0.001, 0.002])],
            ),
            ("shapes::outer::inner::deep", vec![latency([0.001; 4])]),
            ("shapes::signed_consts::-1", vec![latency([0.001; 4])]),
            (
                "shapes::signed_consts::0",
                vec![latency([0.001, 0.211, 0.001, 0.003])],
            ),
            (
                "shapes::signed_consts::5",
                vec![latency([0.001, 0.138, 0.001, 0.003])],
            ),
            (
                "shapes::threads_types::u8::t=1",
                vec![latency([0.001, 0.218, 0.001, 0.003])],
            ),
            (
                "shapes::threads_types::u8::t=2",
                vec![latency([0.002, 0.222, 0.002, 0.048])],
            ),
            (
                "shapes::threads_types::u64::t=1",
                vec![latency([0.001, 0.238, 0.001, 0.004])],
            ),
            (
                "shapes::threads_types::u64::t=2",
                vec![latency([0.001, 0.241, 0.001, 0.004])],
            ),
            (
                "shapes::types_and_consts::u8::2",
                vec![latency([0.724, 2.326, 0.726, 0.743])],
            ),
            (
                "shapes::types_and_consts::u8::16",
                vec![latency([0.54, 2.022, 0.543, 0.569])],
            ),
            (
                "shapes::types_and_consts::u32::2",
                vec![latency([0.721, 0.727, 0.724, 0.724])],
            ),
            (
                "shapes::types_and_consts::u32::16",
                vec![latency([0.841, 2.915, 0.851, 0.869])],
            ),
        ]
    }

    /// `fail_no_fail_fast.txt`: 3 benchmarks, 1 skipped.
    fn fail_no_fail_fast() -> Vec<Reading> {
        vec![
            (
                "fail::a_before",
                vec![latency([0.002, 0.233, 0.002, 0.005])],
            ),
            ("ovf::a::t=1", vec![latency([0.0, 0.476, 0.001, 0.01])]),
            ("ovf::a::t=16", vec![latency([0.001, 1.221, 0.125, 0.109])]),
        ]
    }

    /// `all_targets.txt`: 16 benchmarks, 5 skipped.
    #[expect(clippy::too_many_lines, reason = "every benchmark of a real capture")]
    fn all_targets() -> Vec<Reading> {
        vec![
            (
                "alloc::big_alloc",
                vec![
                    latency([51.86 * US, 2.301 * MS, 51.91 * US, 89.46 * US]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [8.388 * MEGA; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [8.388 * MEGA; 4]),
                ],
            ),
            (
                "alloc::box_alloc",
                vec![
                    latency([7.466, 25.81, 7.861, 8.029]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [8.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [8.0; 4]),
                ],
            ),
            ("alloc::no_alloc", vec![latency([0.0, 0.007, 0.002, 0.003])]),
            (
                "alloc::string_format",
                vec![
                    latency([30.6, 156.2, 30.94, 32.36]),
                    row(
                        BYTES_RATE,
                        [359.4 * MEGA, 70.38 * MEGA, 355.4 * MEGA, 339.8 * MEGA],
                    ),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [16.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [8.0; 4]),
                    row(GROWS, [1.0; 4]),
                    row(GROWN, [8.0; 4]),
                ],
            ),
            (
                "alloc::threaded_alloc::t=1",
                vec![
                    latency([11.92, 13.56, 12.04, 12.07]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [256.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [256.0; 4]),
                ],
            ),
            (
                "alloc::threaded_alloc::t=4",
                vec![
                    latency([29.4, 4.001 * US, 59.4, 117.6]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [256.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [256.0; 4]),
                ],
            ),
            (
                "alloc::vec_push_grow::1",
                vec![
                    latency([18.5, 19.99, 18.69, 18.74]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [32.0; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [32.0; 4]),
                ],
            ),
            (
                "alloc::vec_push_grow::100",
                vec![
                    latency([115.5, 724.0, 142.4, 148.9]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [1.024 * KILO; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [32.0; 4]),
                    row(GROWS, [5.0; 4]),
                    row(GROWN, [992.0; 4]),
                ],
            ),
            (
                "alloc::vec_push_grow::10000",
                vec![
                    latency([3.888 * US, 9.356 * US, 4.019 * US, 4.075 * US]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [131.0 * KILO; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [32.0; 4]),
                    row(GROWS, [12.0; 4]),
                    row(GROWN, [131.0 * KILO; 4]),
                ],
            ),
            (
                "alloc::vec_shrink",
                vec![
                    latency([29.28, 69.58, 29.43, 29.81]),
                    row(MAX_ALLOCATIONS, [1.0; 4]),
                    row(MAX_ALLOCATED, [71.93; 4]),
                    row(ALLOCATIONS, [1.0; 4]),
                    row(ALLOCATED, [8.192 * KILO; 4]),
                    row(SHRINKS, [1.0; 4]),
                    row(SHRUNK, [8.184 * KILO; 4]),
                ],
            ),
            ("edge::t=4", vec![latency([0.0, 0.283, 0.002, 0.005])]),
            (
                "edge::weird_args::   padded",
                vec![latency([0.279, 0.282, 0.28, 0.28])],
            ),
            (
                "edge::weird_args::fastest │ slowest",
                vec![latency([0.279, 0.732, 0.28, 0.288])],
            ),
            (
                "edge::weird_args::t=1",
                vec![latency([0.278, 0.762, 0.28, 0.285])],
            ),
            (
                "edge::├─ fake branch",
                vec![latency([0.0, 0.301, 0.002, 0.006])],
            ),
            (
                "fail::a_before",
                vec![latency([0.001, 0.002, 0.002, 0.001])],
            ),
        ]
    }
}
