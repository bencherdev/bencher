use std::collections::BTreeMap;

use bencher_json::project::{
    measure::built_in::{
        BuiltInMeasure as _,
        default::Latency,
        dotnet::{
            Allocated, Gen0CollectsPer1000Ops, Gen1CollectsPer1000Ops, Gen2CollectsPer1000Ops,
        },
    },
    report::Adapter,
};
use serde::{Deserialize, de::IgnoredAny};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        finite::{Finite, finite_json},
        leaf::{LeafResults, Variant},
        percentile::percentile_name,
    },
    results::adapter_results::AdapterResults,
};

/// `BenchmarkDotNet`'s JSON exports, brief and full, each benchmark on a variant of its arguments,
/// its parameters, its `@job`, and the `@runtime` the job names.
///
/// Each benchmark has its latency, its memory, and every other metric of the full export in a
/// measure of its own. Adapter keys start with `@`, following JSON-LD's convention for keywords,
/// and a parameter spelled like one is skipped, as is a case printed like one already read.
pub struct AdapterCSharpDotNetV1;

impl Adaptable for AdapterCSharpDotNetV1 {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let DotNet { benchmarks, .. } = serde_json::from_str(&finite_json(input)).ok()?;
        let mut results = LeafResults::default();
        for benchmark in benchmarks {
            let Benchmark {
                display_info,
                full_name,
                namespace,
                r#type,
                method,
                method_title,
                parameters,
                statistics,
                memory,
                metrics,
            } = benchmark;
            let name = if let Some(full_name) = &full_name {
                full_name
                    .split_once('(')
                    .map_or(full_name.as_str(), |(name, _)| name)
                    .to_owned()
            } else {
                [
                    namespace.as_deref(),
                    r#type.as_deref(),
                    Some(method.as_str()),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(".")
            };
            let (Some(statistics), Ok(benchmark_name)) = (statistics, name.parse()) else {
                results.skip_benchmark();
                continue;
            };
            let Some(mut parameters) = pairs(&parameters) else {
                results.skip_variant();
                continue;
            };
            for key in [JOB, RUNTIME] {
                if parameters.remove(key).is_some() {
                    results.skip_variant();
                }
            }
            if let Some((job, runtime)) = display_info.as_deref().and_then(|display_info| {
                job(display_info, r#type.as_deref().zip(method_title.as_deref()))
            }) {
                parameters.insert(JOB.to_owned(), job.into());
                if let Some(runtime) = runtime {
                    parameters.insert(RUNTIME.to_owned(), runtime.into());
                }
            }
            let Ok(parameters) = serde_json::from_value(serde_json::Value::Object(parameters))
            else {
                results.skip_variant();
                continue;
            };
            let Some(mut variant) = results.new_variant(benchmark_name, parameters) else {
                continue;
            };
            statistics.insert(&mut variant);
            if let Some(memory) = memory {
                memory.insert(&mut variant);
            }
            for metric in metrics {
                metric.insert(&mut variant);
            }
        }
        results.finish_with_skips(Adapter::CSharpDotNet)
    }
}

/// The job's id and the runtime it names, which `DisplayInfo` writes after the type, the method's
/// title, and `": "`: the id, then the job's characteristics in parentheses, then the benchmark's
/// parameters in brackets.
fn job<'a>(
    display_info: &'a str,
    type_and_title: Option<(&str, &str)>,
) -> Option<(&'a str, Option<&'a str>)> {
    let rest = type_and_title
        .and_then(|(r#type, title)| display_info.strip_prefix(&format!("{type}.{title}: ")))
        .or_else(|| display_info.split_once(": ").map(|(_, rest)| rest))?;
    let end = [rest.find('('), rest.find(" [")]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(rest.len());
    let (id, rest) = rest.split_at_checked(end)?;
    (!id.is_empty()).then(|| (id, rest.strip_prefix('(').and_then(runtime)))
}

const JOB: &str = "@job";
const RUNTIME: &str = "@runtime";

/// The value of the last `Runtime=` among a job's characteristics, which `BenchmarkDotNet` joins
/// with `", "` up to the `)` before the parameters or the end, escaping nothing, after the
/// environment variables that may hold `, Runtime=` themselves.
fn runtime(characteristics: &str) -> Option<&str> {
    let end = characteristics
        .match_indices(')')
        .map(|(index, _)| index)
        .find(|index| {
            characteristics
                .get(index + 1..)
                .is_some_and(|next| next.is_empty() || next.starts_with(" ["))
        })?;
    split_before_names(characteristics.get(..end)?, ", ")
        .filter_map(|characteristic| characteristic.strip_prefix("Runtime="))
        .last()
}

/// The `Name=Value` pairs `BenchmarkDotNet` joins with `&` in `Parameters`, each value a string as
/// it prints it, split only at an `&` before the next pair's name, or `None` if a pair has no `=`
/// or a name repeats. `BenchmarkDotNet` escapes nothing there, so a value holding `&Name=` reads as
/// two pairs.
fn pairs(parameters: &str) -> Option<serde_json::Map<String, serde_json::Value>> {
    let mut pairs = serde_json::Map::new();
    for pair in split_before_names(parameters, "&") {
        let (name, value) = pair.split_once('=')?;
        if pairs.insert(name.to_owned(), value.into()).is_some() {
            return None;
        }
    }
    Some(pairs)
}

/// `text` split at each `separator` before a C# identifier and `=`, and nothing if it is empty.
fn split_before_names<'a>(text: &'a str, separator: &str) -> impl Iterator<Item = &'a str> {
    let mut rest = (!text.is_empty()).then_some(text);
    std::iter::from_fn(move || {
        let text = rest?;
        let end = text
            .match_indices(separator)
            .map(|(index, _)| index)
            .find(|index| {
                text.get(index + separator.len()..)
                    .and_then(|next| next.split_once('='))
                    .is_some_and(|(name, _)| is_identifier(name))
            });
        if let Some(end) = end {
            rest = text.get(end + separator.len()..);
            text.get(..end)
        } else {
            rest = None;
            Some(text)
        }
    })
}

/// Whether `text` can be a C# identifier, as the name of an argument, a parameter, or a
/// characteristic is. Any non-ASCII character may follow the first, where C# also accepts
/// connectors, combining marks, and format characters.
fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first.is_alphabetic() || first == '_')
        && chars.all(|next| next.is_ascii_alphanumeric() || next == '_' || !next.is_ascii())
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DotNet {
    #[serde(rename = "HostEnvironmentInfo")]
    _host_environment_info: IgnoredAny,
    benchmarks: Vec<Benchmark>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Benchmark {
    display_info: Option<String>,
    full_name: Option<String>,
    namespace: Option<String>,
    r#type: Option<String>,
    method: String,
    method_title: Option<String>,
    #[serde(default)]
    parameters: String,
    statistics: Option<Statistics>,
    memory: Option<Memory>,
    #[serde(default)]
    metrics: Vec<Metric>,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct Statistics {
    min: Finite,
    q1: Finite,
    median: Finite,
    mean: Finite,
    q3: Finite,
    max: Finite,
    lower_fence: Finite,
    upper_fence: Finite,
    interquartile_range: Finite,
    standard_deviation: Finite,
    standard_error: Finite,
    confidence_interval: Option<ConfidenceInterval>,
    percentiles: BTreeMap<String, Finite>,
}

impl Statistics {
    /// Every statistic in nanoseconds.
    fn insert(self, variant: &mut Variant<'_>) {
        let Self {
            min,
            q1,
            median,
            mean,
            q3,
            max,
            lower_fence,
            upper_fence,
            interquartile_range,
            standard_deviation,
            standard_error,
            confidence_interval,
            percentiles,
        } = self;
        let latency = Latency::name_id();
        for (percent, value) in percentiles {
            if let Some(name) = percent.strip_prefix('P').and_then(percentile_name) {
                value.insert(variant, &latency, &name, 1.0);
            } else {
                variant.skip_metric();
            }
        }
        for (name, value) in [
            ("min", min),
            ("p25", q1),
            ("median", median),
            ("mean", mean),
            ("p75", q3),
            ("max", max),
            ("lower_fence", lower_fence),
            ("upper_fence", upper_fence),
        ] {
            value.insert(variant, &latency, name, 1.0);
        }
        for (name, spread) in [
            ("iqr", interquartile_range),
            ("std_dev", standard_deviation),
            ("std_err", standard_error),
        ]
        .into_iter()
        .chain(
            confidence_interval
                .map(ConfidenceInterval::spreads)
                .into_iter()
                .flatten(),
        ) {
            spread.insert_spread(mean, variant, &latency, name, 1.0);
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ConfidenceInterval {
    level: Option<serde_json::Value>,
    #[serde(default)]
    lower: Finite,
    #[serde(default)]
    upper: Finite,
    #[serde(default)]
    margin: Finite,
}

impl ConfidenceInterval {
    /// The bounds and the margin of the mean's interval.
    ///
    /// They are named for the interval's level where its ordinal can only mean one level: before
    /// 0.10.4 every interval is 95% and its ordinal 6, and since then every interval is 99.9% and
    /// its ordinal 12, which the earlier levels never reach.
    fn spreads(self) -> [(&'static str, Finite); 3] {
        let Self {
            level,
            lower,
            upper,
            margin,
        } = self;
        let [lower_name, upper_name, margin_name] =
            match level.as_ref().and_then(serde_json::Value::as_u64) {
                Some(6) => ["mean_ci95_lower", "mean_ci95_upper", "mean_ci95_margin"],
                Some(12) => [
                    "mean_ci99_9_lower",
                    "mean_ci99_9_upper",
                    "mean_ci99_9_margin",
                ],
                _ => ["mean_ci_lower", "mean_ci_upper", "mean_ci_margin"],
            };
        [
            (lower_name, lower),
            (upper_name, upper),
            (margin_name, margin),
        ]
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Memory {
    gen0_collections: Finite,
    gen1_collections: Finite,
    gen2_collections: Finite,
    total_operations: Finite,
    bytes_allocated_per_operation: Finite,
}

impl Memory {
    /// The bytes allocated per operation, and `BenchmarkDotNet`'s collections per 1000 operations
    /// of each generation, its total collections over its total operations, in its order.
    fn insert(self, variant: &mut Variant<'_>) {
        let Self {
            gen0_collections,
            gen1_collections,
            gen2_collections,
            total_operations,
            bytes_allocated_per_operation,
        } = self;
        if bytes_allocated_per_operation.get().is_none() {
            variant.skip_metric();
        } else if let Some(allocated) = variant.named(Allocated::SLUG_STR, Allocated::UNITS_STR) {
            bytes_allocated_per_operation.insert(variant, &allocated, "mean", 1.0);
        }
        let Some(operations) = total_operations
            .get()
            .filter(|operations| *operations > 0.0)
        else {
            return;
        };
        for (measure, collections) in [
            (Gen0CollectsPer1000Ops::name_id(), gen0_collections),
            (Gen1CollectsPer1000Ops::name_id(), gen1_collections),
            (Gen2CollectsPer1000Ops::name_id(), gen2_collections),
        ] {
            collections
                .map(|collections| collections / operations * 1_000.0)
                .insert(variant, &measure, "mean", 1.0);
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Metric {
    value: Finite,
    descriptor: Descriptor,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Descriptor {
    id: String,
    unit: Option<String>,
}

impl Metric {
    /// A metric the memory does not already hold, in a measure of its id and in its unit.
    fn insert(self, variant: &mut Variant<'_>) {
        let Self {
            value,
            descriptor: Descriptor { id, unit },
        } = self;
        if MEMORY_METRICS.contains(&id.as_str()) {
            return;
        }
        if let Some(measure) = variant.named(&id, unit.as_deref().unwrap_or_default()) {
            value.insert(variant, &measure, "value", 1.0);
        }
    }
}

/// The ids of the full export's metrics that repeat the memory's.
const MEMORY_METRICS: [&str; 4] = [
    "Gen0Collects",
    "Gen1Collects",
    "Gen2Collects",
    "Allocated Memory",
];

#[cfg(test)]
mod test_c_sharp_dot_net_v1 {
    use bencher_json::{
        BenchmarkName, BenchmarkNameId, ParameterSet,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::Latency,
                dotnet::{
                    Allocated, Gen0CollectsPer1000Ops, Gen1CollectsPer1000Ops,
                    Gen2CollectsPer1000Ops,
                },
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_units, assert_variant, convert_edited_v1,
            convert_v1, variants,
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    /// The statistics `BenchmarkDotNet` writes in nanoseconds, in the order min, the 25th, 50th,
    /// 67th, 80th, 85th, 90th, and 95th percentiles, max, mean, the 75th percentile, standard
    /// deviation, interquartile range, lower fence, upper fence, and standard error.
    const STATISTICS: [&str; 16] = [
        "min",
        "p25",
        "median",
        "p67",
        "p80",
        "p85",
        "p90",
        "p95",
        "max",
        "mean",
        "p75",
        "std_dev",
        "iqr",
        "lower_fence",
        "upper_fence",
        "std_err",
    ];

    /// The statistics, then the mean's interval under `level`'s names: lower, upper, and margin.
    fn latency(
        statistics: [f64; 16],
        level: &'static str,
        [lower, upper, margin]: [f64; 3],
    ) -> Expected {
        let names = match level {
            "99.9" => [
                "mean_ci99_9_lower",
                "mean_ci99_9_upper",
                "mean_ci99_9_margin",
            ],
            _ => ["mean_ci95_lower", "mean_ci95_upper", "mean_ci95_margin"],
        };
        (
            Latency::SLUG_STR,
            STATISTICS
                .into_iter()
                .zip(statistics)
                .chain(names.into_iter().zip([lower, upper, margin]))
                .collect(),
        )
    }

    /// The bytes allocated per operation, then the collections per 1000 operations of each
    /// generation.
    fn memory(allocated: f64, [gen0, gen1, gen2]: [f64; 3]) -> [Expected; 4] {
        [
            (Allocated::SLUG_STR, vec![("mean", allocated)]),
            (Gen0CollectsPer1000Ops::SLUG_STR, vec![("mean", gen0)]),
            (Gen1CollectsPer1000Ops::SLUG_STR, vec![("mean", gen1)]),
            (Gen2CollectsPer1000Ops::SLUG_STR, vec![("mean", gen2)]),
        ]
    }

    /// The names of the benchmarks, sorted.
    fn names(results: &AdapterResults) -> Vec<String> {
        let mut names = results
            .inner
            .keys()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    /// Kills a leaf that drops the type from the name or the job from the parameters, names a
    /// statistic or the interval inaccurately, drops a percentile, a statistic in nanoseconds, or
    /// a metric of the full export, repeats the memory's metrics as measures of their own, takes
    /// raw collections for collections per 1000 operations, gives a measure named at run time no
    /// unit, or records no leaf.
    #[test]
    fn adapter_c_sharp_dot_net_v1() {
        let results = convert_v1(Adapter::CSharpDotNet, "c_sharp/dot_net/diagnosers.json");
        assert_eq!(results.resolved_adapter, Some(Adapter::CSharpDotNet));
        assert_eq!(results.inner.len(), 2);
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        let mut expected = vec![latency(
            [
                3_856.615_379_333_496,
                3_880.129_421_234_131,
                3_903.643_463_134_765_6,
                3_907.490_667_724_609_5,
                3_910.432_647_705_078,
                3_911.564_178_466_797,
                3_912.695_709_228_515_6,
                3_913.827_239_990_234_4,
                3_914.958_770_751_953,
                3_891.739_204_406_738_3,
                3_909.301_116_943_359_4,
                30.939_802_236_757_796,
                29.171_695_709_228_516,
                3_836.371_877_670_288,
                3_953.058_660_507_202,
                17.863_103_150_065_9,
            ],
            "99.9",
            [
                3_327.282_035_347_353,
                4_456.196_373_466_124,
                564.457_169_059_385_3,
            ],
        )];
        expected.extend(memory(224.0, [0.022_888_183_593_75, 0.0, 0.0]));
        expected.extend([
            ("CompletedWorkItemCount", vec![("value", 0.0)]),
            ("LockContentionCount", vec![("value", 0.0)]),
            ("ExceptionFrequency", vec![("value", 1.0)]),
        ]);
        assert_variant(
            &results,
            "Samples.Diagnosed.Throw",
            r#"{"@job":"ShortRun"}"#,
            &expected,
        );
        assert_units(
            &results,
            &[
                (Allocated::SLUG_STR, Allocated::UNITS_STR),
                ("CompletedWorkItemCount", "Count"),
                ("LockContentionCount", "Count"),
                ("ExceptionFrequency", "Count"),
            ],
        );
    }

    /// Kills a leaf that drops a parameter, reads a value as a number, keeps the job's
    /// characteristics or the parameters in the job, or merges two jobs of one benchmark.
    #[test]
    fn adapter_c_sharp_dot_net_v1_parameters() {
        let results = convert_v1(Adapter::CSharpDotNet, "c_sharp/dot_net/params_jobs.json");
        assert_eq!(results.inner.len(), 2);
        assert_eq!(variants(&results), 24);
        let mut expected = vec![latency(
            [
                0.881_834_013_387_560_8,
                0.885_678_395_628_929_1,
                0.889_522_777_870_297_4,
                0.896_048_267_781_734_5,
                0.901_038_348_302_245_3,
                0.902_957_610_040_903_1,
                0.904_876_871_779_561,
                0.906_796_133_518_219,
                0.908_715_395_256_877,
                0.893_357_395_504_911_8,
                0.899_119_086_563_587_2,
                0.013_844_868_800_525_715,
                0.013_440_690_934_658_05,
                0.865_517_359_226_942_1,
                0.919_280_122_965_574_3,
                0.007_993_338_728_878_573,
            ],
            "99.9",
            [
                0.640_775_449_785_200_7,
                1.145_939_341_224_622_8,
                0.252_581_945_719_711,
            ],
        )];
        expected.extend(memory(0.0, [0.0; 3]));
        assert_variant(
            &results,
            "Samples.Lists.Sum",
            r#"{"@job":"Gc","Label":"a lon(...)cters [45]","Size":"16","step":"3"}"#,
            &expected,
        );
    }

    /// Kills a leaf that splits a parameter's value at every `&`, cuts the job only at its
    /// characteristics, which keeps the parameters in a job that has none, or drops a parameter
    /// the user named `job`.
    #[test]
    fn adapter_c_sharp_dot_net_v1_parameter_shapes() {
        let results = convert_v1(Adapter::CSharpDotNet, "c_sharp/dot_net/shapes.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        let mut expected = vec![latency(
            [
                0.0,
                0.0,
                0.0,
                0.0,
                0.000_191_461_667_418_480_94,
                0.000_861_577_503_383_16,
                0.001_980_147_510_766_982_3,
                0.005_727_461_539_208_879,
                0.012_880_111_113_190_651,
                0.001_099_963_982_899_983_7,
                0.0,
                0.003_335_430_456_348_875,
                0.0,
                0.0,
                0.0,
                0.000_861_204_440_658_146_4,
            ],
            "99.9",
            [
                -0.002_465_813_483_907_629,
                0.004_665_741_449_707_597,
                0.003_565_777_466_807_613,
            ],
        )];
        expected.extend(memory(0.0, [0.0; 3]));
        assert_variant(
            &results,
            "Samples.Labels.Length",
            r#"{"@job":"DefaultJob","Label":"x&y","N":"1"}"#,
            &expected,
        );
        let jobs = results
            .inner
            .iter()
            .filter(|(name, _)| name.to_string() == "Samples.Jobs.Length")
            .flat_map(|(_, variants)| variants.keys().map(ParameterSet::canonical))
            .collect::<Vec<_>>();
        assert_eq!(jobs, [r#"{"@job":"ShortRun","job":"mine"}"#]);
    }

    /// The benchmarks and the canonical form of each of their variants, sorted.
    fn variant_keys(results: &AdapterResults) -> Vec<(String, String)> {
        let mut keys = results
            .inner
            .iter()
            .flat_map(|(name, variants)| {
                variants
                    .keys()
                    .map(|parameters| (name.to_string(), parameters.canonical()))
            })
            .collect::<Vec<_>>();
        keys.sort();
        keys
    }

    /// Kills a leaf that reads the parameters from the full name, cuts the name at the last `(`,
    /// or reads a job after the first `": "` of a title that holds one. A value holding `&name=`
    /// reads as two parameters, and a long value as `BenchmarkDotNet` truncates it.
    #[test]
    fn adapter_c_sharp_dot_net_v1_parameters_as_printed() {
        let results = convert_v1(Adapter::CSharpDotNet, "c_sharp/dot_net/literals.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        let shapes = |array: &str| {
            format!(
                r#"{{"@job":"Dry","array":"{array}","d":"1.5","mode":"A, B","type":"Dictionary<String, Int32>"}}"#
            )
        };
        assert_eq!(
            variant_keys(&results),
            sorted([
                (
                    "Probe.Labels.Args",
                    r#"{"@job":"Dry","Label":"a","n":"1","s":"a(b","x":"1"}"#.to_owned(),
                ),
                (
                    "Probe.Labels.Args",
                    r#"{"@job":"Dry","Label":"a","e":"f","n":"3","s":"c)d","x":"1"}"#.to_owned(),
                ),
                (
                    "Probe.Labels.Titled",
                    r#"{"@job":"Dry","Label":"a","x":"1"}"#.to_owned(),
                ),
                (
                    "Probe.Literals.Escaped",
                    r#"{"@job":"Dry","s":"q\"b\\\\s"}"#.to_owned(),
                ),
                (
                    "Probe.Literals.Quoted",
                    r#"{"@job":"Dry","q":"'"}"#.to_owned(),
                ),
                (
                    "Probe.Literals.Scalars",
                    r#"{"@job":"Dry","c":"c","color":"Green","flag":"True","longValue":"a val(...)olumn [40]","none":"?"}"#.to_owned(),
                ),
                ("Probe.Literals.Shapes", shapes("Int32[3]")),
                ("Probe.Literals.Shapes", shapes("Int32[7]")),
                (
                    "Probe.Literals.Slashed",
                    r#"{"@job":"Dry","s":"t\\\\n"}"#.to_owned(),
                ),
            ])
        );
    }

    /// Kills a leaf that reads the parameters from the full name, which prints every array by its
    /// first five elements, `null` as the string `"null"`, and a value with a space in a form it
    /// cannot read. A value holding `&name=` reads as two parameters.
    #[test]
    fn adapter_c_sharp_dot_net_v1_parameters_from_display() {
        let results = convert_v1(Adapter::CSharpDotNet, "c_sharp/dot_net/parameters.json");
        let keys = variant_keys(&results)
            .into_iter()
            .filter(|(name, _)| name != "Probe.Truncated.Run")
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            sorted([
                (
                    "Probe.Arrays.Sum",
                    r#"{"@job":"Dry","N":"?","a":"Int32[1000]"}"#.to_owned(),
                ),
                (
                    "Probe.Arrays.Sum",
                    r#"{"@job":"Dry","N":"?","a":"Int32[100]"}"#.to_owned(),
                ),
                (
                    "Probe.Arrays.Sum",
                    r#"{"@job":"Dry","N":"null","a":"Int32[1000]"}"#.to_owned(),
                ),
                (
                    "Probe.Arrays.Sum",
                    r#"{"@job":"Dry","N":"null","a":"Int32[100]"}"#.to_owned(),
                ),
                (
                    "Probe.Queries.Run",
                    r#"{"@job":"Dry","Query":"a=1","b":"2"}"#.to_owned(),
                ),
                (
                    "Probe.Spaced.Run",
                    r#"{"@job":"Dry","o":"with space"}"#.to_owned(),
                ),
            ])
        );
    }

    /// Kills a leaf that splits a name off only at an ASCII letter, digit, or `_` after its first
    /// character, which merges a C# name holding a connector or a combining mark into the value
    /// before it, or that splits at a name starting with a digit, or rejects a name starting with
    /// a letter that is not ASCII or with `_`, or holding `_` or a digit.
    #[test]
    fn adapter_c_sharp_dot_net_v1_identifiers() {
        let results = convert_v1(Adapter::CSharpDotNet, "c_sharp/dot_net/identifiers.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_eq!(
            variant_keys(&results),
            sorted([
                (
                    "Probe.Names.Run",
                    r#"{"@job":"Dry","S":"a&1=2","step_size":"1","_lead":"2","Été":"3","größe":"5","n2":"6"}"#
                        .to_owned(),
                ),
                (
                    "Probe.Unicode.Run",
                    "{\"@job\":\"Dry\",\"A\":\"1\",\"a\u{203f}b\":\"2\",\"ce\u{301}\":\"3\",\"Zed\":\"4\"}"
                        .to_owned(),
                ),
            ])
        );
    }

    /// Kills a leaf that drops the runtime a job names, which merges a job run on two runtimes,
    /// reads it only as the first characteristic, past its own, or up to the first or the last
    /// `)`, takes a parameter named `Runtime` for the job's, even where the job has no
    /// characteristics, or gives a job that names none a runtime.
    #[test]
    fn adapter_c_sharp_dot_net_v1_runtime() {
        let results = convert_v1(Adapter::CSharpDotNet, "c_sharp/dot_net/runtimes.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_eq!(
            variant_keys(&results),
            sorted([
                (
                    "Probe.Attributed.Add",
                    r#"{"@job":".NET 8.0","@runtime":".NET 8.0"}"#.to_owned(),
                ),
                (
                    "Probe.Custom.Run",
                    r#"{"@job":"Custom","@runtime":".NET 8.0 (custom)","P":"b)"}"#.to_owned(),
                ),
                (
                    "Probe.Later.Add",
                    r#"{"@job":"Dry","@runtime":".NET 9.0"}"#.to_owned(),
                ),
                (
                    "Probe.Named.Run",
                    r#"{"@job":"Dry","A":"1","Runtime":"x)"}"#.to_owned(),
                ),
                (
                    "Probe.Plain.Run",
                    r#"{"@job":"Plain","A":"1","Runtime":"x) ["}"#.to_owned(),
                ),
                ("Probe.Runtimes.Add", r#"{"@job":"Dry"}"#.to_owned()),
                (
                    "Probe.Runtimes.Add",
                    r#"{"@job":"Dry","@runtime":".NET 8.0"}"#.to_owned(),
                ),
                (
                    "Probe.Runtimes.Add",
                    r#"{"@job":"Dry","@runtime":".NET 9.0"}"#.to_owned(),
                ),
            ])
        );
    }

    /// Kills a leaf that reads the first `Runtime=`, finds it inside a characteristic, splits at
    /// every `", "`, or ends at `)[`. A job with no runtime whose environment variable holds
    /// `, Runtime=` reads it as its runtime: `BenchmarkDotNet` prints it as a job that names one.
    #[test]
    fn adapter_c_sharp_dot_net_v1_runtime_environment() {
        let results = convert_v1(
            Adapter::CSharpDotNet,
            "c_sharp/dot_net/runtime_environment.json",
        );
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_eq!(
            variant_keys(&results),
            sorted([
                (
                    "Probe.Environment.Add",
                    r#"{"@job":"Comma","@runtime":".NET 8.0 (a, b)"}"#.to_owned(),
                ),
                (
                    "Probe.Environment.Add",
                    r#"{"@job":"EnvComma","@runtime":".NET 8.0"}"#.to_owned(),
                ),
                (
                    "Probe.Environment.Add",
                    r#"{"@job":"EnvComma","@runtime":"evil"}"#.to_owned(),
                ),
                ("Probe.Environment.Add", r#"{"@job":"EnvKey"}"#.to_owned()),
                (
                    "Probe.Environment.Add",
                    r#"{"@job":"EnvKey","@runtime":".NET 8.0"}"#.to_owned(),
                ),
                (
                    "Probe.Environment.Add",
                    r#"{"@job":"EnvParen","@runtime":".NET 8.0"}"#.to_owned(),
                ),
            ])
        );
    }

    /// Kills a leaf that ends a job's characteristics at the last `)` before ` [` or the end
    /// rather than the first, which reads a runtime from a parameter's value holding `) [`.
    #[test]
    fn adapter_c_sharp_dot_net_v1_runtime_before_parameters() {
        let results = convert_v1(
            Adapter::CSharpDotNet,
            "c_sharp/dot_net/runtime_parameter.json",
        );
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_eq!(
            variant_keys(&results),
            sorted([(
                "Probe.Paren.Length",
                r#"{"@job":"Dry","@runtime":".NET 8.0","P":"a, Runtime=x) [b"}"#.to_owned(),
            )])
        );
    }

    /// Kills a leaf that lets a case printed like one it already read overwrite that variant,
    /// as two long values `BenchmarkDotNet` truncates alike do, or skips it uncounted.
    #[test]
    fn adapter_c_sharp_dot_net_v1_case_printed_alike() {
        let results = convert_v1(Adapter::CSharpDotNet, "c_sharp/dot_net/parameters.json");
        assert_eq!(
            results.skipped,
            Skipped {
                variants: 1,
                ..Skipped::default()
            }
        );
        assert_eq!(variants(&results), 7);
        let means = results
            .inner
            .iter()
            .filter(|(name, _)| name.to_string() == "Probe.Truncated.Run")
            .flat_map(|(_, variants)| variants.iter())
            .map(|(parameters, metrics)| {
                (
                    parameters.canonical(),
                    metrics
                        .inner
                        .iter()
                        .filter(|(measure, _)| measure.to_string() == Latency::SLUG_STR)
                        .flat_map(|(_, metric)| metric.inner.iter())
                        .find(|(name, _)| name.to_string() == "mean")
                        .map(|(_, value)| value.into_inner()),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            means,
            [(
                r#"{"@job":"Dry","Label":"alpha(...)omega [26]"}"#.to_owned(),
                Some(158_362.0)
            )]
        );
    }

    /// Kills a leaf that keeps one of two values a benchmark's argument and parameter of one name
    /// hold, or skips that variant uncounted.
    #[test]
    fn adapter_c_sharp_dot_net_v1_repeated_names() {
        let results = convert_v1(Adapter::CSharpDotNet, "c_sharp/dot_net/edge.json");
        assert_eq!(
            results.skipped,
            Skipped {
                variants: 1,
                ..Skipped::default()
            }
        );
        assert_eq!(
            variant_keys(&results),
            [(
                "Probe.Edge.Control".to_owned(),
                r#"{"@job":"Dry","Label":"p","s":"\\u0001\\u0085x"}"#.to_owned()
            )]
        );
    }

    /// The expected benchmarks and the canonical form of each of their parameters, in the order of
    /// [`variant_keys`].
    fn sorted<const N: usize>(keys: [(&str, String); N]) -> Vec<(String, String)> {
        let mut keys = keys
            .into_iter()
            .map(|(name, parameters)| {
                (
                    name.to_owned(),
                    parameters
                        .parse::<ParameterSet>()
                        .expect("Failed to parse parameters")
                        .canonical(),
                )
            })
            .collect::<Vec<_>>();
        keys.sort();
        keys
    }

    /// Kills a leaf that keeps a parameter spelled like one of its adapter keys, which then means
    /// the user's value, skips it uncounted, or reserves a key only where the run sets it.
    #[test]
    fn adapter_c_sharp_dot_net_v1_adapter_key() {
        for key in ["@job", "@runtime"] {
            for (display_info, parameters) in
                [(true, r#"{"@job":"Dry","N":"1"}"#), (false, r#"{"N":"1"}"#)]
            {
                let results =
                    convert_edited_v1(Adapter::CSharpDotNet, "c_sharp/dot_net/dry.json", |json| {
                        let benchmark = &mut json["Benchmarks"][0];
                        if !display_info && let Some(benchmark) = benchmark.as_object_mut() {
                            benchmark.remove("DisplayInfo");
                        }
                        benchmark["Parameters"] = format!("{key}=theirs&N=1").into();
                    });
                assert_eq!(
                    results.skipped,
                    Skipped {
                        variants: 1,
                        ..Skipped::default()
                    },
                    "{key} {parameters}"
                );
                assert_eq!(
                    variant_keys(&results),
                    [("Dry.Once.Alloc".to_owned(), parameters.to_owned())],
                    "{key}"
                );
            }
        }
    }

    /// Kills a leaf that names a benchmark without its namespace or its type, or with the type
    /// the JSON writes, which drops a nested type's outer type and merges it with another type
    /// of its name, in every release that writes the full name.
    #[test]
    fn adapter_c_sharp_dot_net_v1_names() {
        assert_eq!(
            names(&convert_v1(
                Adapter::CSharpDotNet,
                "c_sharp/dot_net/joined.json"
            )),
            [
                "Samples.Again.Lists.Fill",
                "Samples.Generic<Int32>.Make",
                "Samples.Generic<String>.Make",
                "Samples.Other+Fill.Run",
            ]
        );
        for suffix in [
            "c_sharp/dot_net/nested_0_12.json",
            "c_sharp/dot_net/nested_0_13.json",
        ] {
            assert_eq!(
                names(&convert_v1(Adapter::CSharpDotNet, suffix)),
                [
                    "Samples.Generic<Int32>.Make",
                    "Samples.Generic<String>.Make",
                    "Samples.Outer+Fill.Run",
                ],
                "{suffix}"
            );
        }
        assert_eq!(
            names(&convert_v1(
                Adapter::CSharpDotNet,
                "c_sharp/dot_net/shapes.json"
            )),
            [
                "Samples.Generic<Int32>.Make",
                "Samples.Generic<String>.Make",
                "Samples.Jobs.Length",
                "Samples.Labels.Length",
                "Samples.Outer+Fill.Run",
            ]
        );
        let results = convert_v1(Adapter::CSharpDotNet, "c_sharp/dot_net/global.json");
        let name = BenchmarkNameId::new_name(
            "Global.Add"
                .parse::<BenchmarkName>()
                .expect("Failed to parse benchmark"),
        );
        assert!(
            results.inner.contains_key(&name),
            "{:?}",
            results.inner.keys()
        );
    }

    /// Kills a leaf that reads operation counts as 32-bit, which fails a payload past 2^32
    /// operations.
    #[test]
    fn adapter_c_sharp_dot_net_v1_operations_past_u32() {
        let results = convert_v1(
            Adapter::CSharpDotNet,
            "c_sharp/dot_net/operations_past_u32.json",
        );
        let mut expected = vec![latency([0.0; 16], "99.9", [0.0; 3])];
        expected.extend(memory(0.0, [0.0; 3]));
        assert_variant(
            &results,
            "Dry.Empty.Nothing",
            r#"{"@job":"Job-ROLVQH"}"#,
            &expected,
        );
    }

    /// Kills a leaf that computes collections per 1000 operations in another order than
    /// `BenchmarkDotNet`, which differs from its own value in the last bit.
    #[test]
    fn adapter_c_sharp_dot_net_v1_collections_in_its_order() {
        let results =
            convert_edited_v1(Adapter::CSharpDotNet, "c_sharp/dot_net/dry.json", |json| {
                let memory = &mut json["Benchmarks"][0]["Memory"];
                memory["Gen0Collections"] = 1.into();
                memory["TotalOperations"] = 123_456_789.into();
            });
        let gen0 = results
            .inner
            .values()
            .flat_map(|variants| variants.values())
            .flat_map(|metrics| metrics.inner.iter())
            .filter(|(measure, _)| measure.to_string() == Gen0CollectsPer1000Ops::SLUG_STR)
            .flat_map(|(_, metric)| metric.inner.values())
            .map(|value| value.into_inner().to_bits())
            .collect::<Vec<_>>();
        assert_eq!(gen0, [(1.0f64 / 123_456_789.0 * 1_000.0).to_bits()]);
    }

    /// Kills a leaf that divides by zero operations, which writes collections that are not a
    /// number.
    #[test]
    fn adapter_c_sharp_dot_net_v1_no_operations() {
        let results =
            convert_edited_v1(Adapter::CSharpDotNet, "c_sharp/dot_net/dry.json", |json| {
                json["Benchmarks"][0]["Memory"]["TotalOperations"] = 0.into();
            });
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        let mut measures = results
            .inner
            .values()
            .flat_map(|variants| variants.values())
            .flat_map(|metrics| metrics.inner.keys().map(ToString::to_string))
            .collect::<Vec<_>>();
        measures.sort();
        assert_eq!(measures, [Allocated::SLUG_STR, Latency::SLUG_STR]);
    }

    /// Kills a leaf that fails the payload on the empty string `BenchmarkDotNet` writes for a value
    /// that is not finite, or on the bare `NaN` releases before 0.13 write, drops a metric beside
    /// one, or counts the interval `BenchmarkDotNet` cannot compute from one sample beside its
    /// mean.
    #[test]
    fn adapter_c_sharp_dot_net_v1_non_finite() {
        for (suffix, statistic, allocated) in [
            ("c_sharp/dot_net/dry.json", 227_509.0, 88.0),
            ("c_sharp/dot_net/dry_0_12.json", 159_974.0, 760.0),
        ] {
            let results = convert_v1(Adapter::CSharpDotNet, suffix);
            assert!(
                results.skipped.is_empty(),
                "{suffix}: {:?}",
                results.skipped
            );
            let mut statistics = [statistic; 16];
            statistics[11] = 0.0;
            statistics[12] = 0.0;
            statistics[15] = 0.0;
            let (measure, metrics) = latency(statistics, "99.9", [0.0; 3]);
            let mut expected = vec![(measure, metrics.into_iter().take(16).collect())];
            expected.extend(memory(allocated, [0.0; 3]));
            assert_variant(&results, "Dry.Once.Alloc", r#"{"@job":"Dry"}"#, &expected);
        }
    }

    /// Kills a leaf that fails the payload on an allocation `BenchmarkDotNet` could not measure,
    /// keeps a measure for it, or skips it uncounted.
    #[test]
    fn adapter_c_sharp_dot_net_v1_unmeasured_allocation() {
        let results =
            convert_edited_v1(Adapter::CSharpDotNet, "c_sharp/dot_net/dry.json", |json| {
                json["Benchmarks"][0]["Memory"]["BytesAllocatedPerOperation"] =
                    serde_json::Value::Null;
            });
        assert_eq!(results.skipped.metrics, 1);
        let measures = results
            .inner
            .values()
            .flat_map(|variants| variants.values())
            .flat_map(|metrics| metrics.inner.keys())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        assert!(
            !measures.contains(&Allocated::SLUG_STR.to_owned()),
            "{measures:?}"
        );
        assert!(
            measures.contains(&Latency::SLUG_STR.to_owned()),
            "{measures:?}"
        );
    }

    /// Kills a leaf that fails the payload on a benchmark with no statistics, which a failed
    /// benchmark writes, or skips it uncounted.
    #[test]
    fn adapter_c_sharp_dot_net_v1_failed() {
        let results = convert_v1(Adapter::CSharpDotNet, "c_sharp/dot_net/failed.json");
        assert_eq!(results.inner.len(), 0);
        assert_eq!(results.skipped.benchmarks, 1);
    }

    /// Kills a leaf that reads the level's ordinal in today's enum, where a release before 0.10.4
    /// writes 6 for its 95% interval, takes the 25th percentile over the first quartile, or
    /// fails a release that writes no full name.
    #[test]
    fn adapter_c_sharp_dot_net_v1_before_0_10_4() {
        let results = convert_v1(Adapter::CSharpDotNet, "c_sharp/dot_net/two.json");
        assert_eq!(results.inner.len(), 2);
        assert_variant(
            &results,
            "BenchmarkDotNet.Samples.Intro.IntroJsonExport.Sleep10",
            "{}",
            &[latency(
                [
                    10_265_993.720_937_5,
                    10_337_369.528_437_499,
                    10_360_382.695_312_5,
                    10_373_496.555_659_376,
                    10_400_703.484_187_5,
                    10_417_280.326_718_75,
                    10_424_125.595_812_501,
                    10_435_620.516_093_75,
                    10_436_008.320_937_5,
                    10_362_283.085_796_878,
                    10_392_063.158_906_251,
                    45_702.118_369_205_156,
                    54_693.630_468_752_235,
                    10_255_329.082_734_37,
                    10_474_103.604_609_38,
                    10_219.304_338_928_456,
                ],
                "95",
                [
                    10_342_253.249_292_579,
                    10_382_312.922_301_177,
                    20_029.836_504_299_772,
                ],
            )],
        );
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_c_sharp_dot_net_v1_ignores_average() {
        for suffix in [
            "c_sharp/dot_net/diagnosers.json",
            "c_sharp/dot_net/params_jobs.json",
            "c_sharp/dot_net/shapes.json",
        ] {
            assert_ignores_average(Adapter::CSharpDotNet, suffix);
        }
    }
}
