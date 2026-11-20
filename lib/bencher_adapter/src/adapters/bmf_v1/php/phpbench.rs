use std::{cmp::Ordering, f64::consts::PI};

use bencher_json::{
    BenchmarkName,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Latency, PeakMemory, Throughput},
        },
        report::Adapter,
    },
};
use quick_xml::{
    events::{BytesStart, Event},
    reader::Reader,
};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        cpp::catch2_xml::attribute,
        finite::Finite,
        leaf::{LeafResults, Variant, parameter_set},
    },
    results::adapter_results::AdapterResults,
};

/// `PHPBench` 1.x's `--dump-file` XML: each subject named `Class::subject` on a variant of its
/// parameter set, with the statistics `PHPBench` writes over its per-revolution times
/// (`XmlEncoder.php:119-203`, 1.7.0).
pub struct AdapterPhpPhpbench;

impl Adaptable for AdapterPhpPhpbench {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut reader = Reader::from_str(input);
        let mut dump = Dump::default();
        let mut path = Vec::new();
        loop {
            match reader.read_event().ok()? {
                Event::Start(element) => {
                    let opened = dump.open(&element, path.last())?;
                    path.push(opened);
                },
                Event::Empty(element) => {
                    let opened = dump.open(&element, path.last())?;
                    dump.close(opened);
                },
                Event::End(_) => dump.close(path.pop()?),
                Event::Eof => break,
                Event::Text(_)
                | Event::CData(_)
                | Event::Comment(_)
                | Event::Decl(_)
                | Event::PI(_)
                | Event::DocType(_)
                | Event::GeneralRef(_) => {},
            }
        }
        if !path.is_empty() {
            return None;
        }
        dump.results.finish_with_skips(Adapter::PhpPhpbench)
    }
}

#[derive(Default)]
struct Dump {
    results: LeafResults,
    class: Option<String>,
    name: Option<BenchmarkName>,
    variant: Option<VariantDump>,
}

/// An element the leaf reads, where `PHPBench` writes it.
#[derive(Clone, Copy)]
enum Element {
    Root,
    Suite,
    Benchmark,
    Subject,
    Variant,
    ParameterSet,
    Parameter,
    Other,
}

impl Dump {
    /// The element opened inside `parent`, or `None` for a document that is not a `PHPBench` 1.x
    /// dump.
    fn open(&mut self, element: &BytesStart<'_>, parent: Option<&Element>) -> Option<Element> {
        let Some(parent) = parent else {
            return (element.name().as_ref() == b"phpbench" && is_version_1(element))
                .then_some(Element::Root);
        };
        Some(match (parent, element.name().as_ref()) {
            (Element::Root, b"suite") => Element::Suite,
            (Element::Suite, b"benchmark") => {
                self.class = attribute(element, "class");
                Element::Benchmark
            },
            (Element::Benchmark, b"subject") => {
                self.name = self
                    .class
                    .as_deref()
                    .zip(attribute(element, "name"))
                    .and_then(|(class, subject)| {
                        let class = class.strip_prefix('\\').unwrap_or(class);
                        format!("{class}::{subject}").parse().ok()
                    });
                Element::Subject
            },
            (Element::Subject, b"variant") => {
                self.variant = Some(VariantDump::new(element));
                Element::Variant
            },
            (Element::Variant, b"parameter-set") => Element::ParameterSet,
            (Element::ParameterSet | Element::Parameter, b"parameter") => {
                if let Some(variant) = &mut self.variant {
                    variant.parameters.open(element);
                }
                Element::Parameter
            },
            (Element::Variant, b"iteration") => {
                if let Some(variant) = &mut self.variant {
                    variant.read_iteration(element);
                }
                Element::Other
            },
            (Element::Variant, b"stats") => {
                if let Some(variant) = &mut self.variant {
                    variant.stats = Some(Stats::new(element));
                }
                Element::Other
            },
            _ => Element::Other,
        })
    }

    fn close(&mut self, element: Element) {
        match element {
            Element::Parameter => {
                if let Some(variant) = &mut self.variant {
                    variant.parameters.close();
                }
            },
            Element::Variant => {
                if let Some(variant) = self.variant.take() {
                    variant.insert(&mut self.results, self.name.clone());
                }
            },
            Element::Root
            | Element::Suite
            | Element::Benchmark
            | Element::Subject
            | Element::ParameterSet
            | Element::Other => {},
        }
    }
}

/// `PHPBench` 0.x writes its tag as the root `version`, or the literal `@git_tag@` when Composer
/// installed it (`PhpBench.php:26`, `XmlEncoder.php:43`, 0.17.1), where 1.x writes the version
/// Composer installed (`PhpBench.php:252-265`, 1.7.0).
fn is_version_1(element: &BytesStart<'_>) -> bool {
    attribute(element, "version")
        .is_some_and(|version| !version.starts_with("0.") && version != "@git_tag@")
}

struct VariantDump {
    output_mode: OutputMode,
    parameters: Parameters,
    stats: Option<Stats>,
    memory: Option<Vec<f64>>,
}

/// How `PHPBench` displays a variant's times: `throughput` shows each as its rate.
enum OutputMode {
    Time,
    Throughput,
}

impl VariantDump {
    fn new(element: &BytesStart<'_>) -> Self {
        Self {
            output_mode: if attribute(element, "output-mode").as_deref() == Some("throughput") {
                OutputMode::Throughput
            } else {
                OutputMode::Time
            },
            parameters: Parameters::default(),
            stats: None,
            memory: Some(Vec::new()),
        }
    }

    fn read_iteration(&mut self, element: &BytesStart<'_>) {
        let Some(peak) = attribute(element, "mem-peak") else {
            return;
        };
        if let Some(memory) = &mut self.memory
            && let Some(peak) = Finite::from(peak.parse().ok()).get()
        {
            memory.push(peak);
        } else {
            self.memory = None;
        }
    }

    /// A variant without `<stats>`, as one whose subject threw and wrote `<errors>` instead
    /// (`XmlEncoder.php:140-152`), or with a parameter Bencher cannot hold, is skipped and counted.
    fn insert(self, results: &mut LeafResults, name: Option<BenchmarkName>) {
        let Self {
            output_mode,
            parameters,
            stats,
            memory,
        } = self;
        let (Some(name), Some(parameters), Some(stats)) =
            (name, parameters.set.and_then(parameter_set), stats)
        else {
            results.skip_variant();
            return;
        };
        let Some(mut variant) = results.new_variant(name, parameters) else {
            return;
        };
        stats.insert(&mut variant, &output_mode);
        insert_memory(&mut variant, memory);
    }
}

/// A variant's statistics of its per-revolution times, in microseconds whatever
/// `output-time-unit` says (`Variant.php:166-178`, `TimeResult.php:24`).
struct Stats {
    mean: Finite,
    min: Finite,
    max: Finite,
    mode: Finite,
    std_dev: Finite,
}

impl Stats {
    fn new(element: &BytesStart<'_>) -> Self {
        let stat =
            |name| Finite::from(attribute(element, name).and_then(|value| value.parse().ok()));
        Self {
            mean: stat("mean"),
            min: stat("min"),
            max: stat("max"),
            mode: stat("mode"),
            std_dev: stat("stdev"),
        }
    }

    /// The times as `latency`, and in throughput mode also as the rates `PHPBench` displays,
    /// `1 / time` (`TimeUnit.php:281-296`), so that the fastest time is the highest rate.
    fn insert(&self, variant: &mut Variant<'_>, output_mode: &OutputMode) {
        let Self {
            mean,
            min,
            max,
            mode,
            std_dev,
        } = *self;
        let latency = Latency::name_id();
        for (metric, value) in [("mean", mean), ("min", min), ("max", max), ("mode", mode)] {
            value.insert(variant, &latency, metric, NANOS_PER_MICRO);
        }
        std_dev.insert_spread(mean, variant, &latency, "std_dev", NANOS_PER_MICRO);
        if let OutputMode::Throughput = output_mode {
            let throughput = Throughput::name_id();
            for (metric, time) in [("max", min), ("min", max), ("mode", mode)] {
                Finite::from(time.get().map(|time| MICROS_PER_SECOND / time)).insert(
                    variant,
                    &throughput,
                    metric,
                    1.0,
                );
            }
        }
    }
}

const NANOS_PER_MICRO: f64 = 1_000.0;
const MICROS_PER_SECOND: f64 = 1_000_000.0;

/// `peak-memory` from each iteration's `mem-peak`, as `PHPBench`'s reports compute it: `max` as its
/// aggregate report does (`ExpressionGenerator.php:93`), and `mode` and `std_dev` as its benchmark
/// report's memory chart does (`generators.php:229-230`).
fn insert_memory(variant: &mut Variant<'_>, memory: Option<Vec<f64>>) {
    let Some(memory) = memory else {
        variant.skip_measure();
        return;
    };
    let Some(max) = memory.iter().copied().reduce(f64::max) else {
        return;
    };
    let measure = PeakMemory::name_id();
    variant.insert(measure.clone(), "max", max);
    for (metric, value) in [
        ("mode", kde_mode(&memory)),
        ("std_dev", std_dev(&memory, Divisor::Population)),
    ] {
        if let Some(value) = value {
            variant.insert(measure.clone(), metric, value);
        } else {
            variant.skip_metric();
        }
    }
}

/// `PHPBench`'s mode: the peak of a Gaussian kernel density estimate with Scott's bandwidth over
/// 512 points from the minimum to the maximum, the mean of the points where the peak ties
/// (`Statistics.php:101-160`, `Kde.php:107-241`), in `PHPBench`'s order of operations so that a
/// tie breaks as it does.
fn kde_mode(population: &[f64]) -> Option<f64> {
    let min = population.iter().copied().reduce(f64::min)?;
    let max = population.iter().copied().reduce(f64::max)?;
    if equal(min, max) {
        return Some(min);
    }
    let count = f64::from(u32::try_from(population.len()).ok()?);
    let factor = count.powf(-1.0 / 5.0);
    let data_covariance = std_dev(population, Divisor::Sample)?.powi(2);
    let covariance = data_covariance * factor.powi(2);
    let inverse_covariance = (1.0 / data_covariance) / factor.powi(2);
    let norm = (2.0 * PI * covariance).sqrt() * count;
    let unit = (max - min) / f64::from(KDE_POINTS - 1);
    let mut points = Vec::new();
    let mut point = min;
    while point <= max {
        // A step below the spacing of floats near `max` never reaches it.
        if points.len() == usize::from(KDE_POINTS) {
            return None;
        }
        points.push(point);
        point += unit;
    }
    let densities = points
        .iter()
        .map(|point| {
            population.iter().fold(0.0, |sum, value| {
                let diff = value - point;
                sum + (-(diff * (inverse_covariance * diff) / 2.0)).exp()
            }) / norm
        })
        .collect::<Vec<_>>();
    let peak = densities.iter().copied().reduce(f64::max)?;
    let modes = points
        .iter()
        .zip(&densities)
        .filter_map(|(point, density)| equal(*density, peak).then_some(*point))
        .collect::<Vec<_>>();
    let count = f64::from(u32::try_from(modes.len()).ok()?);
    Some(modes.iter().fold(0.0, |sum, mode| sum + mode) / count)
}

const KDE_POINTS: u16 = 512;

/// `PHPBench`'s standard deviation (`Statistics.php:27-82`).
fn std_dev(values: &[f64], divisor: Divisor) -> Option<f64> {
    let count = u32::try_from(values.len()).ok()?;
    let mean = values.iter().fold(0.0, |sum, value| sum + value) / f64::from(count);
    let squares = values
        .iter()
        .fold(0.0, |sum, value| sum + (value - mean).powi(2));
    let divisor = match divisor {
        Divisor::Population => count,
        Divisor::Sample => count.checked_sub(1)?,
    };
    Some((squares / f64::from(divisor)).sqrt())
}

#[derive(Clone, Copy)]
enum Divisor {
    Population,
    Sample,
}

/// PHP's `==` on two floats.
fn equal(left: f64, right: f64) -> bool {
    left.partial_cmp(&right) == Some(Ordering::Equal)
}

/// A variant's `<parameter-set>` as bare keys under the user's names, without the set's name, or
/// none once a value is one a parameter cannot hold.
struct Parameters {
    set: Option<serde_json::Map<String, serde_json::Value>>,
    open: Vec<(String, Value)>,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            set: Some(serde_json::Map::new()),
            open: Vec::new(),
        }
    }
}

impl Parameters {
    fn open(&mut self, element: &BytesStart<'_>) {
        let name = attribute(element, "name").unwrap_or_default();
        self.open.push((name, Value::new(element)));
    }

    /// A nil value leaves its key out, and a collection is its compact JSON text.
    fn close(&mut self) {
        let Some((name, value)) = self.open.pop() else {
            return;
        };
        let value = match value {
            Value::Scalar(value) => Some(value),
            Value::Nil => None,
            Value::Collection(entries) => Some(collection(entries)),
            Value::Unreadable => {
                self.set = None;
                return;
            },
        };
        if let Some((_, parent)) = self.open.last_mut() {
            if let Value::Collection(entries) = parent {
                entries.push((name, value.unwrap_or_default()));
            }
        } else if let Some(set) = &mut self.set
            && let Some(value) = value
        {
            let value = if value.is_array() || value.is_object() {
                value.to_string().into()
            } else {
                value
            };
            set.insert(name, value);
        }
    }
}

/// A parameter as `PHPBench` types it (`XmlEncoder.php:205-265`).
enum Value {
    Scalar(serde_json::Value),
    Nil,
    Collection(Vec<(String, serde_json::Value)>),
    /// A `binary` or `serialized` value, written base64, which `PHPBench` writes as nil unless
    /// `storage.store_binary` is set (`StorageExtension.php:50`), or a double that is not finite.
    Unreadable,
}

impl Value {
    fn new(element: &BytesStart<'_>) -> Self {
        if attribute(element, "xsi:nil").as_deref() == Some("true") {
            return Self::Nil;
        }
        let scalar = match (
            attribute(element, "type").as_deref(),
            attribute(element, "value"),
        ) {
            (Some("collection"), _) => return Self::Collection(Vec::new()),
            (Some("integer"), Some(value)) => value.parse::<i64>().ok().map(Into::into),
            (Some("double"), Some(value)) => value
                .parse()
                .ok()
                .and_then(serde_json::Number::from_f64)
                .map(Into::into),
            (Some("boolean"), Some(value)) => Some((value == "1").into()),
            (Some("string"), Some(value)) => Some(value.into()),
            _ => None,
        };
        scalar.map_or(Self::Unreadable, Self::Scalar)
    }
}

/// A PHP array as `json_encode` writes it: a list when its keys run from 0 in order, else an
/// object.
fn collection(entries: Vec<(String, serde_json::Value)>) -> serde_json::Value {
    if entries
        .iter()
        .enumerate()
        .all(|(index, (key, _))| *key == index.to_string())
    {
        entries.into_iter().map(|(_, value)| value).collect()
    } else {
        entries
            .into_iter()
            .collect::<serde_json::Map<_, _>>()
            .into()
    }
}

#[cfg(test)]
mod test_php_phpbench {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{Latency, PeakMemory, Throughput},
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use super::kde_mode;
    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_units, assert_value, assert_variant,
            convert_v1, value, variants,
        },
        results::adapter_results::Skipped,
    };

    const MEMORY: &str = "App\\Bench\\MemoryBench::benchAllocate";
    const SHAPES: &str = "App\\Bench\\ParamsBench::benchShapes";

    /// Kills a leaf that keeps the class's leading backslash, leaves the times in microseconds,
    /// drops `mode`, reads `rstdev` as `std_dev`, writes a rate outside throughput mode, types a
    /// boolean as its text, keeps a nil key, writes a list as an object, or records another leaf.
    #[test]
    fn adapter_php_phpbench_dump() {
        let results = convert_v1(Adapter::PhpPhpbench, "php/phpbench/dump.xml");
        assert_eq!(results.resolved_adapter, Some(Adapter::PhpPhpbench));
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(results.inner.len(), 8);
        assert_eq!(variants(&results), 12);
        assert_variant(
            &results,
            "HashBench::benchMd5",
            "{}",
            &[
                latency([102.4, 88.0, 139.0, 92.890_410_958_904, 19.168_724_527_208]),
                memory([661_944.0, 661_944.0, 0.0]),
            ],
        );
        assert_variant(
            &results,
            "HashBench::benchSha256",
            "{}",
            &[
                latency([225.4, 207.0, 241.0, 231.751_467_710_37, 12.208_193_969_625]),
                memory([661_944.0, 661_944.0, 0.0]),
            ],
        );
        assert_variant(
            &results,
            "HashBench::benchTyped",
            r#"{"flag":true,"label":"a \"quoted\" label","list":"[1,2]","ratio":0.5,"size":10}"#,
            &[
                latency([24.2, 21.0, 32.0, 22.270_058_708_415, 4.069_397_989_875_2]),
                memory([663_256.0, 663_256.0, 0.0]),
            ],
        );
        assert_value(
            &results,
            (
                "HashBench::benchTyped",
                r#"{"flag":false,"label":"plain","list":"[3]","ratio":1.5,"size":10000}"#,
            ),
            (Latency::SLUG_STR, "mean"),
            85.0,
        );
        for (parameters, mean) in [
            (r#"{"mode":"repeat","n":10}"#, 36.8),
            (r#"{"mode":"repeat","n":100}"#, 36.4),
            (r#"{"mode":"fill","n":10}"#, 47.4),
            (r#"{"mode":"fill","n":100}"#, 101.4),
        ] {
            assert_value(
                &results,
                ("HashBench::benchCartesian", parameters),
                (Latency::SLUG_STR, "mean"),
                mean,
            );
        }
        assert_variant(
            &results,
            "App\\Bench\\Nested\\MemoryBench::benchOnce",
            "{}",
            &[
                latency([1_062_000.0, 1_062_000.0, 1_062_000.0, 1_062_000.0, 0.0]),
                memory([661_960.0, 661_960.0, 0.0]),
            ],
        );
        assert_units(&results, &[]);
    }

    /// Kills a leaf that writes no rate in throughput mode, takes the rate's extremes from the
    /// same extremes of the times, or inverts the mean.
    #[test]
    fn adapter_php_phpbench_throughput() {
        let results = convert_v1(Adapter::PhpPhpbench, "php/phpbench/dump.xml");
        assert_variant(
            &results,
            "HashBench::benchThroughput",
            "{}",
            &[
                latency([22.0, 20.0, 30.0, 20.019_569_471_624, 4.0]),
                rate([50_000_000.0, 33_333_333.333_333_336, 49_951_124.144_673_19]),
                memory([661_944.0, 661_944.0, 0.0]),
            ],
        );

        let results = convert_v1(Adapter::PhpPhpbench, "php/phpbench/shapes.xml");
        assert_variant(
            &results,
            "App\\Bench\\RateBench::benchRate",
            "{}",
            &[
                latency([104.4, 103.0, 106.0, 104.086_105_675_15, 1.019_803_902_718_6]),
                rate([
                    9_708_737.864_077_67,
                    9_433_962.264_150_944,
                    9_607_430.247_423_932,
                ]),
                memory([662_664.0, 662_664.0, 0.0]),
            ],
        );
    }

    /// Kills a `peak-memory` whose `max` is the smallest `mem-peak`, whose `std_dev` is a sample
    /// standard deviation, or whose `mode` takes Silverman's bandwidth, against `PHPBench`'s own
    /// report over the same run.
    #[test]
    fn adapter_php_phpbench_memory() {
        let results = convert_v1(Adapter::PhpPhpbench, "php/phpbench/shapes.xml");
        for (parameters, expected) in [
            (
                r#"{"kib":"[100,100,100,200,400,100,700]"}"#,
                [1_247_464.0, 691_121.299_412_916_4, 206_432.355_635_111_7],
            ),
            (
                r#"{"kib":"[100,300]"}"#,
                [837_296.0, 833_887.780_821_941, 86_186.848_184_627_34],
            ),
            (
                r#"{"kib":"[100,200,300]"}"#,
                [837_304.0, 817_195.506_849_336, 73_609.946_089_270_48],
            ),
        ] {
            for (metric, value) in ["max", "mode", "std_dev"].into_iter().zip(expected) {
                assert_value(
                    &results,
                    (MEMORY, parameters),
                    (PeakMemory::SLUG_STR, metric),
                    value,
                );
            }
        }
    }

    /// Kills a mode that takes Silverman's bandwidth, a population covariance, or a grid of 513
    /// points, against `PHPBench`'s own mode of every variant's per-revolution times, which
    /// `<stats>` rounds to 14 significant digits.
    #[test]
    fn adapter_php_phpbench_kde_mode() {
        let mut checked = 0;
        for fixture in ["dump.xml", "big.xml", "fail.xml", "baseline.xml"] {
            let input = std::fs::read_to_string(format!("./tool_output/php/phpbench/{fixture}"))
                .expect("Failed to read a fixture");
            for variant in input.split("<variant ").skip(1) {
                let revs = number(variant, "revs=\"");
                let times = variant
                    .split("<iteration ")
                    .skip(1)
                    .map(|iteration| number(iteration, "time-net=\"") / revs)
                    .collect::<Vec<_>>();
                let Some(stats) = variant.split("<stats ").nth(1) else {
                    continue;
                };
                let expected = number(stats, "mode=\"");
                let mode = kde_mode(&times).expect("Failed to find the mode");
                assert!(
                    (mode - expected).abs() <= expected.abs() * 1e-12,
                    "{fixture}: {mode} != {expected}"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 45);
    }

    /// Kills a mode that takes the first of 2 tied grid points rather than their mean, against
    /// `PHPBench`'s own `Statistics::kdeMode([0, 511])`.
    #[test]
    fn adapter_php_phpbench_kde_mode_tie() {
        assert_eq!(kde_mode(&[0.0, 511.0]), Some(255.5));
    }

    /// Kills a leaf that keeps a variant past the key cap, a double that is not finite as its
    /// text, or a repeat of a variant already read (2 sets alike once their nil keys are dropped,
    /// and one subject at 2 revolution counts), refuses a nil value, or writes a keyed collection
    /// as a list.
    #[test]
    fn adapter_php_phpbench_parameters() {
        let results = convert_v1(Adapter::PhpPhpbench, "php/phpbench/shapes.xml");
        assert_eq!(
            results.skipped,
            Skipped {
                variants: 4,
                ..Skipped::default()
            }
        );
        assert_eq!(variants(&results), 10);
        for (parameters, mean) in [
            (r#"{"n":1}"#, 0.8),
            (r#"{"n":2}"#, 1.4),
            (
                r#"{"empty":"[]","map":"{\"a\":[true,null,\"x\",1.5,-0.25],\"b\":1}","n":3,"sparse":"{\"2\":\"two\",\"5\":\"five\"}"}"#,
                1.85,
            ),
            (r#"{"n":5}"#, 0.55),
            (
                r#"{"k01":1,"k02":2,"k03":3,"k04":4,"k05":5,"k06":6,"k07":7,"k08":8,"k09":9,"k10":10,"k11":11,"k12":12,"k13":13,"k14":14,"k15":15,"k16":16}"#,
                1.1,
            ),
        ] {
            assert_value(
                &results,
                (SHAPES, parameters),
                (Latency::SLUG_STR, "mean"),
                mean * 1_000.0,
            );
        }
        assert_value(
            &results,
            ("App\\Bench\\RateBench::benchRevs", "{}"),
            (Latency::SLUG_STR, "mean"),
            280.0,
        );
    }

    /// Kills a leaf that writes `peak-memory`, or counts a skip, for an executor that measures no
    /// memory.
    #[test]
    fn adapter_php_phpbench_local_executor() {
        let results = convert_v1(Adapter::PhpPhpbench, "php/phpbench/local.xml");
        assert_eq!(results.skipped, Skipped::default());
        assert_variant(
            &results,
            "App\\Bench\\RateBench::benchRate",
            "{}",
            &[
                latency([128.8, 127.0, 134.0, 127.520_547_945_21, 2.638_181_191_654_6]),
                rate([
                    1_000_000.0 / 0.127,
                    1_000_000.0 / 0.134,
                    1_000_000.0 / 0.127_520_547_945_21,
                ]),
            ],
        );
    }

    /// Kills a rate written for a time of 0 as `PHPBench` displays it, a rate of 0.
    #[test]
    fn adapter_php_phpbench_zero_time() {
        let results = convert_v1(Adapter::PhpPhpbench, "php/phpbench/zero.xml");
        assert_eq!(
            results.skipped,
            Skipped {
                metrics: 2,
                ..Skipped::default()
            }
        );
        let benchmark = ("App\\Bench\\ZeroBench::benchNothing", "{}");
        assert_value(
            &results,
            benchmark,
            (Throughput::SLUG_STR, "min"),
            500_000.0,
        );
        assert_value(&results, benchmark, (Latency::SLUG_STR, "min"), 0.0);
        for metric in ["max", "mode"] {
            assert_eq!(
                value(
                    &results,
                    benchmark.0,
                    benchmark.1,
                    Throughput::SLUG_STR,
                    metric
                ),
                None,
                "{metric}"
            );
        }
    }

    /// Kills a `mem-peak` that is not finite read as a number, or dropped uncounted. `PHPBench`
    /// writes every `mem-peak` as an integer, so the test edits a real capture.
    #[test]
    fn adapter_php_phpbench_unreadable_memory() {
        let input = std::fs::read_to_string("./tool_output/php/phpbench/dump.xml")
            .expect("Failed to read a fixture")
            .replace("mem-peak=\"661960\"", "mem-peak=\"1e999\"");
        let results = Adapter::PhpPhpbench
            .convert(&input, Settings::new(None, BmfVersion::V1))
            .expect("Failed to parse the edited dump");
        assert_eq!(
            results.skipped,
            Skipped {
                measures: 1,
                ..Skipped::default()
            }
        );
        assert_variant(
            &results,
            "App\\Bench\\Nested\\MemoryBench::benchOnce",
            "{}",
            &[latency([
                1_062_000.0,
                1_062_000.0,
                1_062_000.0,
                1_062_000.0,
                0.0,
            ])],
        );
    }

    /// Kills a mode whose grid never ends, for `mem-peak` values closer than its step can resolve,
    /// rather than a mode skipped and counted. `PHPBench` measures no such memory, so the test
    /// edits a real capture.
    #[test]
    fn adapter_php_phpbench_unresolved_mode() {
        let input = std::fs::read_to_string("./tool_output/php/phpbench/dump.xml")
            .expect("Failed to read a fixture")
            .replacen("mem-peak=\"1560672\"", "mem-peak=\"9007199254740992\"", 2)
            .replacen("mem-peak=\"1560672\"", "mem-peak=\"9007199254740994\"", 1);
        let results = Adapter::PhpPhpbench
            .convert(&input, Settings::new(None, BmfVersion::V1))
            .expect("Failed to parse the edited dump");
        assert_eq!(
            results.skipped,
            Skipped {
                metrics: 1,
                ..Skipped::default()
            }
        );
        let benchmark = ("App\\Bench\\Nested\\MemoryBench::benchAllocate", "{}");
        assert_value(
            &results,
            benchmark,
            (PeakMemory::SLUG_STR, "max"),
            9_007_199_254_740_994.0,
        );
        assert_eq!(
            value(
                &results,
                benchmark.0,
                benchmark.1,
                PeakMemory::SLUG_STR,
                "mode"
            ),
            None
        );
    }

    /// Kills a leaf that reads the variants of a dump cut short. The test cuts a real capture.
    #[test]
    fn adapter_php_phpbench_cut() {
        let input = std::fs::read_to_string("./tool_output/php/phpbench/dump.xml")
            .expect("Failed to read a fixture");
        let cut = input
            .find("<subject name=\"benchThroughput\">")
            .and_then(|index| input.get(..index))
            .expect("Failed to cut the dump");
        assert_eq!(
            Adapter::PhpPhpbench.convert(cut, Settings::new(None, BmfVersion::V1)),
            None
        );
    }

    /// Kills a leaf that keeps a variant whose `binary` or `serialized` value `PHPBench` stored.
    #[test]
    fn adapter_php_phpbench_binary() {
        let results = convert_v1(Adapter::PhpPhpbench, "php/phpbench/binary.xml");
        assert_eq!(
            results.skipped,
            Skipped {
                variants: 5,
                ..Skipped::default()
            }
        );
        assert_eq!(variants(&results), 3);
    }

    /// Kills a leaf that skips a variant whose subject threw uncounted, or drops a variant that
    /// failed only its `#[Assert]`.
    #[test]
    fn adapter_php_phpbench_fail() {
        let results = convert_v1(Adapter::PhpPhpbench, "php/phpbench/fail.xml");
        assert_eq!(
            results.skipped,
            Skipped {
                variants: 1,
                ..Skipped::default()
            }
        );
        assert_eq!(variants(&results), 2);
        assert_variant(
            &results,
            "FailBench::benchAssertFails",
            "{}",
            &[
                latency([
                    57_243.333_333_333,
                    56_760.0,
                    57_790.0,
                    57_112.739_726_028,
                    422.873_766_297_02,
                ]),
                memory([661_944.0, 661_944.0, 0.0]),
            ],
        );
    }

    /// Kills a leaf that reads `<baseline-stats>`.
    #[test]
    fn adapter_php_phpbench_baseline() {
        let results = convert_v1(Adapter::PhpPhpbench, "php/phpbench/baseline.xml");
        assert_variant(
            &results,
            "HashBench::benchMd5",
            "{}",
            &[
                latency([94.8, 92.0, 97.0, 96.090_019_569_471, 1.939_071_942_966_5]),
                memory([661_944.0, 661_944.0, 0.0]),
            ],
        );
    }

    /// Kills a leaf that reads `PHPBench` 0.x, by a Composer install's `@git_tag@` or by a release
    /// tag.
    #[test]
    fn adapter_php_phpbench_refuses_version_0() {
        let input = std::fs::read_to_string("./tool_output/php/phpbench/v0_17_1.xml")
            .expect("Failed to read a fixture");
        // A PHAR of 0.x writes its release tag where a Composer install writes `@git_tag@`.
        let tagged = input.replace("version=\"@git_tag@\"", "version=\"0.17.1\"");
        assert_ne!(tagged, input);
        for input in [input.as_str(), &tagged] {
            for adapter in [Adapter::PhpPhpbench, Adapter::Magic] {
                assert_eq!(
                    adapter.convert(input, Settings::new(None, BmfVersion::V1)),
                    None,
                    "{adapter}"
                );
            }
        }
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_php_phpbench_ignores_average() {
        assert_ignores_average(Adapter::PhpPhpbench, "php/phpbench/dump.xml");
    }

    /// `latency` in nanoseconds: `mean`, `min`, `max`, `mode`, and `std_dev`.
    fn latency([mean, min, max, mode, std_dev]: [f64; 5]) -> Expected {
        (
            Latency::SLUG_STR,
            vec![
                ("mean", mean),
                ("min", min),
                ("max", max),
                ("mode", mode),
                ("std_dev", std_dev),
            ],
        )
    }

    /// `throughput` per second: `max`, `min`, and `mode`.
    fn rate([max, min, mode]: [f64; 3]) -> Expected {
        (
            Throughput::SLUG_STR,
            vec![("max", max), ("min", min), ("mode", mode)],
        )
    }

    /// `peak-memory` in bytes: `max`, `mode`, and `std_dev`.
    fn memory([max, mode, std_dev]: [f64; 3]) -> Expected {
        (
            PeakMemory::SLUG_STR,
            vec![("max", max), ("mode", mode), ("std_dev", std_dev)],
        )
    }

    /// The number in quotes after `key` in `text`.
    fn number(text: &str, key: &str) -> f64 {
        text.split(key)
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(|| panic!("no {key} in {text}"))
    }
}
