use std::borrow::Cow;

use bencher_json::{
    BenchmarkName, ParameterSet,
    project::{
        measure::built_in::{BuiltInMeasure as _, default::Latency},
        report::Adapter,
    },
};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, de::IgnoredAny};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{finite::Finite, js::vitest::common_directory, leaf::LeafResults},
    results::adapter_results::AdapterResults,
};

/// The report of `deno bench --json`, at its schema version 1.
///
/// A bench is named by its file path, decoded from its URL, relative to the common directory of
/// every file in the report, its group, and its name. Only a bench's first result is read: Deno
/// adds a result to a bench for each bench of the same name.
pub struct AdapterJsDeno;

impl Adaptable for AdapterJsDeno {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let Report {
            version, benches, ..
        } = serde_json::from_str(input).ok()?;
        if version != 1 {
            return None;
        }
        let origins = benches
            .iter()
            .map(|bench| {
                percent_decode_str(&bench.origin)
                    .decode_utf8()
                    .unwrap_or(Cow::Borrowed(&bench.origin))
            })
            .collect::<Vec<_>>();
        let root = common_directory(origins.iter().map(AsRef::as_ref));
        let mut results = LeafResults::default();
        for (bench, origin) in benches.iter().zip(&origins) {
            let mut bench_results = bench.results.iter();
            let first = bench_results.next();
            for _ in bench_results {
                results.skip_variant();
            }
            let path = origin.get(root..).unwrap_or(origin);
            let name = [
                Some(path),
                bench.group.as_deref(),
                Some(bench.name.as_str()),
            ]
            .into_iter()
            .flatten()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" > ");
            let (Some(BenchResult::Ok(stats)), Ok(benchmark_name)) =
                (first, name.parse::<BenchmarkName>())
            else {
                results.skip_benchmark();
                continue;
            };
            let Some(mut variant) = results.new_variant(benchmark_name, ParameterSet::default())
            else {
                continue;
            };
            let latency = Latency::name_id();
            for (metric, value) in [
                ("mean", stats.avg),
                ("min", stats.min),
                ("max", stats.max),
                ("p75", stats.p75),
                ("p99", stats.p99),
                ("p99_5", stats.p995),
                ("p99_9", stats.p999),
            ] {
                value.insert(&mut variant, &latency, metric, 1.0);
            }
        }
        results.finish_with_skips(Adapter::JsDeno)
    }
}

#[derive(Deserialize)]
struct Report {
    version: u64,
    #[serde(rename = "runtime")]
    _runtime: IgnoredAny,
    #[serde(rename = "cpu")]
    _cpu: IgnoredAny,
    benches: Vec<Bench>,
}

#[derive(Deserialize)]
struct Bench {
    origin: String,
    group: Option<String>,
    name: String,
    results: Vec<BenchResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum BenchResult {
    Ok(Stats),
    Failed(IgnoredAny),
}

/// The statistics of a bench's samples, each the time of one iteration in nanoseconds.
#[derive(Deserialize)]
struct Stats {
    avg: Finite,
    min: Finite,
    max: Finite,
    p75: Finite,
    p99: Finite,
    p995: Finite,
    p999: Finite,
}

#[cfg(test)]
mod test_js_deno {
    use std::collections::BTreeSet;

    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{BuiltInMeasure as _, default::Latency},
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            assert_ignores_average, assert_variant, convert_edited_v1, convert_v1,
        },
        results::adapter_results::AdapterResults,
    };

    fn names(results: &AdapterResults) -> BTreeSet<String> {
        results.inner.keys().map(ToString::to_string).collect()
    }

    /// Kills a name with the file's absolute URL, a file not cut at the common directory of every
    /// file, a group dropped or put after the name, a failed bench kept or left uncounted, a
    /// second result read in place of the first or left uncounted, and a single file named with
    /// its directory.
    #[test]
    fn adapter_js_deno_names() {
        let results = convert_v1(Adapter::JsDeno, "js/deno/files.json");
        assert_eq!(results.resolved_adapter, Some(Adapter::JsDeno));
        assert_eq!(
            (results.skipped.benchmarks, results.skipped.variants),
            (1, 1)
        );
        assert_eq!(
            names(&results),
            [
                "main_bench.ts > duplicate",
                "main_bench.ts > explicit timers",
                "main_bench.ts > explicit timers fast",
                "main_bench.ts > fill size=10",
                "main_bench.ts > fill size=100",
                "main_bench.ts > fill size=1000",
                "main_bench.ts > logs",
                "main_bench.ts > n and warmup",
                "main_bench.ts > noop",
                "main_bench.ts > parsing > json parse",
                "main_bench.ts > parsing > url parse",
                "main_bench.ts > sleep 20ms",
                "main_bench.ts > sort 1000",
                "sub/deep_bench.ts > noop",
                "sub/deep_bench.ts > parsing > url parse",
            ]
            .map(ToOwned::to_owned)
            .into()
        );
        assert_variant(
            &results,
            "main_bench.ts > duplicate",
            "{}",
            &[(
                Latency::SLUG_STR,
                vec![
                    ("mean", 2.687_865_808_843_366_5),
                    ("min", 2.1903),
                    ("max", 31.317),
                    ("p75", 2.7052),
                    ("p99", 3.4733),
                    ("p99_5", 3.8209),
                    ("p99_9", 5.8981),
                ],
            )],
        );
        let results = convert_v1(Adapter::JsDeno, "js/deno/single.json");
        assert!(results.skipped.is_empty());
        assert_eq!(
            names(&results),
            [
                "deep_bench.ts > noop",
                "deep_bench.ts > parsing > url parse"
            ]
            .map(ToOwned::to_owned)
            .into()
        );
    }

    /// Kills a file named by its URL's percent-escapes, and a common directory taken before the
    /// escapes are decoded, which here holds a space and a non-ASCII letter.
    #[test]
    fn adapter_js_deno_decoded_path() {
        let results = convert_v1(Adapter::JsDeno, "js/deno/paths.json");
        assert!(results.skipped.is_empty());
        assert_eq!(
            names(&results),
            [
                "plain/plain_bench.ts > noop",
                "sub dir/space é_bench.ts > noop"
            ]
            .map(ToOwned::to_owned)
            .into()
        );
    }

    /// Kills an origin whose escapes do not decode to UTF-8, as Linux allows in a file name,
    /// decoded with a replacement character instead of kept as Deno wrote it.
    #[test]
    fn adapter_js_deno_invalid_utf8_path() {
        let results = convert_v1(Adapter::JsDeno, "js/deno/invalid_utf8.json");
        assert!(results.skipped.is_empty());
        assert_eq!(
            names(&results),
            ["bad%FF_bench.ts > noop", "plain_bench.ts > noop"]
                .map(ToOwned::to_owned)
                .into()
        );
    }

    /// Kills a statistic dropped, misnamed, or scaled from Deno's nanoseconds, and a sample count
    /// or flag written as a metric.
    #[test]
    fn adapter_js_deno_statistics() {
        let results = convert_v1(Adapter::JsDeno, "js/deno/quiet.json");
        assert!(results.skipped.is_empty());
        assert_eq!(results.inner.len(), 4);
        assert_variant(
            &results,
            "quiet_bench.ts > sleep 2ms",
            "{}",
            &[(
                Latency::SLUG_STR,
                vec![
                    ("mean", 3_109_566.0),
                    ("min", 2_106_895.0),
                    ("max", 3_222_709.0),
                    ("p75", 3_128_372.0),
                    ("p99", 3_193_454.0),
                    ("p99_5", 3_222_709.0),
                    ("p99_9", 3_222_709.0),
                ],
            )],
        );
    }

    /// Kills a group of the empty string joined as an empty part of the name.
    #[test]
    fn adapter_js_deno_empty_group() {
        let results = convert_edited_v1(Adapter::JsDeno, "js/deno/quiet.json", |json| {
            json["benches"][2]["group"] = "".into();
        });
        assert!(results.get("quiet_bench.ts > url parse").is_some());
    }

    /// Kills a report of another schema version read as version 1.
    #[test]
    fn adapter_js_deno_refuses_another_version() {
        let input = std::fs::read_to_string("./tool_output/js/deno/quiet.json")
            .expect("Failed to read a fixture");
        let mut json: serde_json::Value = serde_json::from_str(&input).expect("Failed to parse");
        json["version"] = 2.into();
        assert_eq!(
            Adapter::JsDeno.convert(&json.to_string(), Settings::new(None, BmfVersion::V1)),
            None
        );
    }

    /// Kills a value Deno writes as `null` read as 0 or left uncounted.
    #[test]
    fn adapter_js_deno_null_statistic() {
        let results = convert_edited_v1(Adapter::JsDeno, "js/deno/quiet.json", |json| {
            json["benches"][0]["results"][0]["ok"]["p999"] = serde_json::Value::Null;
        });
        assert_eq!(results.skipped.metrics, 1);
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_js_deno_ignores_average() {
        for suffix in [
            "js/deno/files.json",
            "js/deno/quiet.json",
            "js/deno/single.json",
        ] {
            assert_ignores_average(Adapter::JsDeno, suffix);
        }
    }
}
