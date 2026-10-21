use bencher_json::project::report::Adapter;
use quick_xml::{
    XmlVersion,
    events::{BytesStart, Event},
    reader::Reader,
};
use rust_decimal::Decimal;

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        cpp::catch2::{Statistics, insert},
        leaf::LeafResults,
    },
    results::adapter_results::AdapterResults,
};

/// Catch2's `--reporter xml`: each `BenchmarkResults` in nanoseconds under its test case and
/// sections, its interval's level stated by `ci`.
pub struct AdapterCppCatch2Xml;

impl Adaptable for AdapterCppCatch2Xml {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut reader = Reader::from_str(input);
        let mut results = LeafResults::default();
        let mut path = Vec::new();
        let mut benchmark = None;
        loop {
            match reader.read_event().ok()? {
                Event::Start(element) => match element.name().as_ref() {
                    b"TestCase" | b"Section" => path.push(attribute(&element, "name")?),
                    b"BenchmarkResults" => {
                        benchmark = Some(Benchmark {
                            name: attribute(&element, "name").unwrap_or_default(),
                            mean: None,
                            std_dev: None,
                        });
                    },
                    _ => {},
                },
                Event::Empty(element) => {
                    if let Some(benchmark) = &mut benchmark {
                        match element.name().as_ref() {
                            b"mean" => benchmark.mean = Estimate::new(&element),
                            b"standardDeviation" => benchmark.std_dev = Estimate::new(&element),
                            _ => {},
                        }
                    }
                },
                Event::End(element) => match element.name().as_ref() {
                    b"TestCase" | b"Section" => {
                        path.pop();
                    },
                    b"BenchmarkResults" => {
                        if let Some(benchmark) = benchmark.take() {
                            benchmark.insert(&mut results, &path);
                        }
                    },
                    _ => {},
                },
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
        results.finish_with_skips(Adapter::CppCatch2Xml)
    }
}

struct Benchmark {
    name: String,
    mean: Option<Estimate>,
    std_dev: Option<Estimate>,
}

impl Benchmark {
    /// A benchmark without a mean, as one that failed, is skipped and counted.
    fn insert(self, results: &mut LeafResults, path: &[String]) {
        let Self {
            name,
            mean,
            std_dev,
        } = self;
        let Some(mean) = mean else {
            results.skip_benchmark();
            return;
        };
        let level = mean.level();
        let statistics = Statistics {
            mean: mean.values(),
            std_dev: std_dev
                .filter(|_| !matches!(level, Some(Level::NoAnalysis)))
                .map(|std_dev| std_dev.values()),
            level: level.as_ref().and_then(Level::name),
        };
        insert(results, path, &name, &statistics);
    }
}

/// A statistic and its bootstrap interval, at the confidence level `ci`.
struct Estimate {
    value: f64,
    lower: f64,
    upper: f64,
    ci: Option<Decimal>,
}

impl Estimate {
    fn new(element: &BytesStart<'_>) -> Option<Self> {
        let number = |name| attribute(element, name)?.parse::<f64>().ok();
        Some(Self {
            value: number("value")?,
            lower: number("lowerBound")?,
            upper: number("upperBound")?,
            ci: attribute(element, "ci").and_then(|ci| ci.parse().ok()),
        })
    }

    fn values(&self) -> [f64; 3] {
        [self.value, self.lower, self.upper]
    }

    /// Catch2 writes a level of 0 when `--benchmark-no-analysis` skipped the bootstrap.
    fn level(&self) -> Option<Level> {
        let ci = self.ci?;
        Some(if ci.is_zero() {
            Level::NoAnalysis
        } else {
            Level::Percent(
                (ci * Decimal::ONE_HUNDRED)
                    .normalize()
                    .to_string()
                    .replace('.', "_"),
            )
        })
    }
}

enum Level {
    NoAnalysis,
    Percent(String),
}

impl Level {
    fn name(&self) -> Option<&str> {
        match self {
            Self::NoAnalysis => None,
            Self::Percent(percent) => Some(percent),
        }
    }
}

fn attribute(element: &BytesStart<'_>, name: &str) -> Option<String> {
    element
        .try_get_attribute(name)
        .ok()??
        .normalized_value(XmlVersion::default())
        .ok()
        .map(Into::into)
}

#[cfg(test)]
mod test_cpp_catch2_xml {
    use bencher_json::project::report::Adapter;
    use pretty_assertions::assert_eq;

    use crate::adapters::bmf_v1::{
        cpp::catch2::test_cpp_catch2_v1::{Names, assert_v3, latency, mean},
        test_util::{assert_ignores_average, assert_variant, convert_v1},
    };

    const CI95: Names = [
        "mean",
        "mean_ci95_lower",
        "mean_ci95_upper",
        "std_dev",
        "std_dev_ci95_lower",
        "std_dev_ci95_upper",
    ];

    /// Kills a leaf that drops a section, names a benchmark without its test case, takes the
    /// bounds as anything but Catch2's bootstrap interval, leaves the stated level out of the
    /// interval names, drops the standard deviation, claims a failed benchmark or skips it
    /// uncounted, or records no leaf.
    #[test]
    fn adapter_cpp_catch2_v1_xml_v3() {
        for adapter in [Adapter::CppCatch2, Adapter::CppCatch2Xml] {
            let results = convert_v1(adapter, "cpp/catch2/v3.xml");
            assert_eq!(
                results.resolved_adapter,
                Some(Adapter::CppCatch2Xml),
                "{adapter}"
            );
            assert_v3(
                &results,
                CI95,
                [
                    [
                        2.59346,
                        2.57228,
                        2.67213,
                        0.082_690_5,
                        0.011_738_6,
                        0.164_084,
                    ],
                    [46.5289, 45.432, 49.4866, 3.70243, 0.066_759_7, 7.04777],
                    [78.4679, 75.945, 82.65, 7.27893, 5.07172, 10.0078],
                    [24.4795, 24.109, 26.1772, 1.51736, 0.211_782, 3.01349],
                    [3_946.89, 3_920.75, 4_100.11, 112.032, 2.83639, 223.166],
                ],
            );
        }
    }

    /// Kills a leaf that names a level other than the one `ci` states.
    #[test]
    fn adapter_cpp_catch2_v1_xml_ci99() {
        let results = convert_v1(Adapter::CppCatch2Xml, "cpp/catch2/v3_ci99.xml");
        assert_eq!(results.inner.len(), 2);
        assert_variant(
            &results,
            "Fibonacci / Fibonacci 10",
            "{}",
            &[latency(
                [
                    "mean",
                    "mean_ci99_lower",
                    "mean_ci99_upper",
                    "std_dev",
                    "std_dev_ci99_lower",
                    "std_dev_ci99_upper",
                ],
                [23.7187, 23.6618, 23.9118, 0.21335, 0.017_253_7, 0.416_316],
            )],
        );
    }

    /// Kills a leaf that writes the zero interval and standard deviation of a run without
    /// analysis.
    #[test]
    fn adapter_cpp_catch2_v1_xml_no_analysis() {
        let results = convert_v1(Adapter::CppCatch2Xml, "cpp/catch2/v3_no_analysis.xml");
        assert_eq!(results.inner.len(), 2);
        assert_variant(&results, "Fibonacci / Fibonacci 10", "{}", &[mean(23.8379)]);
        assert_variant(
            &results,
            "Fibonacci / Fibonacci 20",
            "{}",
            &[mean(3_967.77)],
        );
    }

    /// Kills a leaf that reads only the v3 document's root or groups.
    #[test]
    fn adapter_cpp_catch2_v1_xml_v2() {
        let results = convert_v1(Adapter::CppCatch2Xml, "cpp/catch2/v2.xml");
        assert_eq!(results.inner.len(), 3);
        assert_variant(
            &results,
            "Fibonacci / Fibonacci 20",
            "{}",
            &[latency(
                CI95,
                [4_020.44, 3_953.42, 4_353.3, 289.991, 2.44481, 576.122],
            )],
        );
        assert_variant(
            &results,
            "Containers / vector / push 64",
            "{}",
            &[latency(
                CI95,
                [74.3835, 70.3768, 81.9561, 12.8189, 5.53031, 19.1033],
            )],
        );
    }

    /// Kills an XML leaf that reads the average setting.
    #[test]
    fn adapter_cpp_catch2_v1_xml_ignores_average() {
        assert_ignores_average(Adapter::CppCatch2Xml, "cpp/catch2/v3.xml");
    }
}
