use std::collections::BTreeSet;

use bencher_adapter::{
    JsonV0Measures, JsonV0Results, JsonV1Measures, JsonV1Results, JsonV1Variant,
    results::adapter_metrics::NamedMap,
};
use bencher_json::{
    BenchmarkNameId, BmfVersion, JsonNewMetric, MeasureNameId, MetricName, ParameterSet,
};
use rand::{
    RngExt as _, SeedableRng as _,
    distr::{Distribution as _, Uniform},
    rngs::StdRng,
};

use crate::{CliError, cli_println, parser::mock::CliMock};

use super::SubCmd;

const DEFAULT_COUNT: usize = 5;

#[derive(Debug, Clone)]
pub struct Mock {
    pub count: Option<usize>,
    pub measures: Vec<MeasureNameId>,
    pub format: MockFormat,
    pub pow: Option<i32>,
    pub fail: bool,
    pub flaky: bool,
    pub seed: Option<u64>,
}

#[expect(clippy::absolute_paths, reason = "error variants wrap external types")]
#[derive(thiserror::Error, Debug)]
pub enum MockError {
    #[error(
        "BMF version 0 results have no parameters: drop `--parameters`, or drop `--bmf-version 0` to write BMF version 1"
    )]
    ParametersAtV0,

    #[error("Failed to generate uniform distribution: {0}")]
    BadDistribution(rand::distr::uniform::Error),

    #[error("Failed to parse benchmark name: {0}")]
    ParseBenchmarkName(bencher_json::ValidError),

    #[error("Failed to serialize mock results: {0}")]
    SerializeResults(serde_json::Error),

    #[error("Mock failure")]
    MockFailure,
}

impl TryFrom<CliMock> for Mock {
    type Error = MockError;

    fn try_from(mock: CliMock) -> Result<Self, Self::Error> {
        let CliMock {
            count,
            measure,
            parameters,
            bmf_version,
            pow,
            fail,
            flaky,
            seed,
        } = mock;
        let format = if bmf_version == BmfVersion::V1 {
            MockFormat::v1(parameters)
        } else if parameters.is_empty() {
            MockFormat::V0
        } else {
            return Err(MockError::ParametersAtV0);
        };
        Ok(Self {
            count,
            measures: measure,
            format,
            pow,
            fail,
            flaky,
            seed,
        })
    }
}

impl SubCmd for Mock {
    fn exec(&self) -> impl Future<Output = Result<(), CliError>> {
        std::future::ready(self.exec_inner().map_err(Into::into))
    }
}

impl Mock {
    fn exec_inner(&self) -> Result<(), MockError> {
        cli_println!("{}", self.generate()?);

        if self.fail || (self.flaky && rand::rng().random::<bool>()) {
            Err(MockError::MockFailure)
        } else {
            Ok(())
        }
    }

    fn generate(&self) -> Result<String, MockError> {
        match &self.format {
            MockFormat::V0 => serde_json::to_string_pretty(&self.generate_v0()?),
            MockFormat::V1 { variants } => {
                serde_json::to_string_pretty(&self.generate_v1(variants)?)
            },
        }
        .map_err(MockError::SerializeResults)
    }

    fn generate_v0(&self) -> Result<JsonV0Results, MockError> {
        let count = self.count.unwrap_or(DEFAULT_COUNT);
        let mut sampler = self.sampler();
        let mut results = JsonV0Results::with_capacity(count);
        for c in 0..count {
            let mut measures_map = JsonV0Measures::with_capacity(self.measures.len());
            for measure in &self.measures {
                let value = sampler.sample(c)?;
                let variance = value * 0.1;
                let metric = JsonNewMetric {
                    value: value.into(),
                    lower_value: Some((value - variance).into()),
                    upper_value: Some((value + variance).into()),
                };
                measures_map.insert(measure.clone(), metric);
            }
            results.insert(benchmark_name(c)?, measures_map);
        }
        Ok(results)
    }

    /// With no parameters, a seed draws the same values as it does at version 0.
    fn generate_v1(&self, variants: &BTreeSet<ParameterSet>) -> Result<JsonV1Results, MockError> {
        let count = self.count.unwrap_or(DEFAULT_COUNT);
        let mut sampler = self.sampler();
        let mut results = JsonV1Results::with_capacity(count);
        for c in 0..count {
            let mut entries = Vec::with_capacity(variants.len());
            for parameters in variants {
                let mut measures = JsonV1Measures::with_capacity(self.measures.len());
                for measure in &self.measures {
                    let value = sampler.sample(c)?;
                    measures.insert(
                        measure.clone(),
                        NamedMap::from([(MetricName::value(), value.into())]),
                    );
                }
                entries.push(JsonV1Variant {
                    parameters: parameters.clone(),
                    measures,
                });
            }
            results.insert(benchmark_name(c)?, entries);
        }
        Ok(results)
    }

    fn sampler(&self) -> Sampler {
        Sampler {
            rng: if let Some(seed) = self.seed {
                StdRng::seed_from_u64(seed)
            } else {
                rand::make_rng()
            },
            ten_pow: 10.0f64.powi(self.pow.unwrap_or(1)),
        }
    }
}

/// The BMF version of the mock results, with what only that version can carry.
#[derive(Debug, Clone)]
pub enum MockFormat {
    V0,
    /// Every benchmark has one variant for each set of parameters.
    V1 {
        variants: BTreeSet<ParameterSet>,
    },
}

impl MockFormat {
    fn v1(parameters: Vec<ParameterSet>) -> Self {
        let mut variants: BTreeSet<ParameterSet> = parameters.into_iter().collect();
        if variants.is_empty() {
            variants.insert(ParameterSet::default());
        }
        Self::V1 { variants }
    }
}

struct Sampler {
    rng: StdRng,
    ten_pow: f64,
}

impl Sampler {
    /// A value in the band of the `c`th benchmark, `[10^pow * c, 10^pow * (c + 1))`.
    #[expect(
        clippy::cast_precision_loss,
        reason = "count index to f64 is fine for mock data"
    )]
    fn sample(&mut self, c: usize) -> Result<f64, MockError> {
        let low = self.ten_pow * c as f64;
        let high = self.ten_pow * (c + 1) as f64;
        let uniform = Uniform::new(low, high).map_err(MockError::BadDistribution)?;
        Ok(uniform.sample(&mut self.rng))
    }
}

fn benchmark_name(c: usize) -> Result<BenchmarkNameId, MockError> {
    format!("bencher::mock_{c}")
        .as_str()
        .parse()
        .map_err(MockError::ParseBenchmarkName)
}

#[cfg(test)]
mod tests {
    use bencher_adapter::{Adaptable as _, AdapterResults, Settings};
    use bencher_json::{BmfVersion, project::report::Adapter};
    use clap::Parser as _;

    use super::{Mock, MockError};
    use crate::parser::mock::CliMock;

    fn mock(args: &[&str]) -> Result<Mock, MockError> {
        Mock::try_from(cli(args))
    }

    fn cli(args: &[&str]) -> CliMock {
        CliMock::try_parse_from(std::iter::once("mock").chain(args.iter().copied()))
            .expect("Failed to parse mock arguments")
    }

    fn generate(args: &[&str]) -> String {
        mock(args)
            .expect("Failed to build mock")
            .generate()
            .expect("Failed to generate mock results")
    }

    /// The results as the server's adapter for `bmf_version` reads them.
    fn read(json: &str, bmf_version: BmfVersion) -> Option<AdapterResults> {
        let adapter = if bmf_version == BmfVersion::V1 {
            Adapter::JsonV1
        } else {
            Adapter::JsonV0
        };
        adapter.convert(json, Settings::new(None, bmf_version))
    }

    /// Every metric as `benchmark parameters measure name`, with its value.
    fn metrics(results: &AdapterResults) -> Vec<(String, f64)> {
        let mut metrics = results
            .inner
            .iter()
            .flat_map(|(benchmark, variants)| {
                variants.iter().flat_map(move |(parameters, measures)| {
                    measures.inner.iter().flat_map(move |(measure, metric)| {
                        metric.inner.iter().map(move |(name, value)| {
                            (
                                format!("{benchmark} {} {measure} {name}", parameters.canonical()),
                                value.into_inner(),
                            )
                        })
                    })
                })
            })
            .collect::<Vec<_>>();
        metrics.sort_by(|a, b| a.0.cmp(&b.0));
        metrics
    }

    fn names(metrics: &[(String, f64)]) -> Vec<&str> {
        metrics.iter().map(|(name, _)| name.as_str()).collect()
    }

    /// Kills: a default that writes version 0, a version 1 measure with any name but `value`,
    /// and a default variant with parameters.
    #[test]
    fn mock_writes_bmf_v1_by_default() {
        let json = generate(&["--seed", "6"]);
        assert_eq!(read(&json, BmfVersion::V0), None);
        let results = read(&json, BmfVersion::V1).expect("Failed to read BMF v1");
        let metrics = metrics(&results);
        assert_eq!(
            names(&metrics),
            [
                "bencher::mock_0 {} latency value",
                "bencher::mock_1 {} latency value",
                "bencher::mock_2 {} latency value",
                "bencher::mock_3 {} latency value",
                "bencher::mock_4 {} latency value",
            ]
        );
    }

    /// Kills: a version 1 band that ignores `--pow` or the benchmark's index, `--count`
    /// ignored, and a measure dropped.
    #[test]
    fn mock_v1_keeps_count_pow_and_measures() {
        let json = generate(&[
            "--count",
            "3",
            "--pow",
            "3",
            "--measure",
            "latency",
            "--measure",
            "throughput",
        ]);
        let results = read(&json, BmfVersion::V1).expect("Failed to read BMF v1");
        let metrics = metrics(&results);
        assert_eq!(
            names(&metrics),
            [
                "bencher::mock_0 {} latency value",
                "bencher::mock_0 {} throughput value",
                "bencher::mock_1 {} latency value",
                "bencher::mock_1 {} throughput value",
                "bencher::mock_2 {} latency value",
                "bencher::mock_2 {} throughput value",
            ]
        );
        for (benchmark, c) in metrics.chunks(2).zip(0u8..) {
            let band = 1000.0 * f64::from(c)..1000.0 * f64::from(c + 1);
            for (name, value) in benchmark {
                assert!(band.contains(value), "{name} = {value}");
            }
        }
    }

    /// Kills: `--parameters` ignored, only the first set kept, a duplicate set written twice,
    /// and the empty variant kept beside the sets.
    #[test]
    fn mock_parameters_make_one_variant_per_set() {
        let json = generate(&[
            "--count",
            "2",
            "--parameters",
            r#"{"size": 64}"#,
            "--parameters",
            r#"{"size": 4096, "op": "read"}"#,
            "--parameters",
            r#"{"size": 64}"#,
        ]);
        let results = read(&json, BmfVersion::V1).expect("Failed to read BMF v1");
        assert_eq!(
            names(&metrics(&results)),
            [
                r#"bencher::mock_0 {"op":"read","size":4096} latency value"#,
                r#"bencher::mock_0 {"size":64} latency value"#,
                r#"bencher::mock_1 {"op":"read","size":4096} latency value"#,
                r#"bencher::mock_1 {"size":64} latency value"#,
            ]
        );
        let raw: serde_json::Value = serde_json::from_str(&json).expect("Failed to parse JSON");
        assert_eq!(raw["bencher::mock_0"].as_array().map(Vec::len), Some(2));
    }

    /// Kills: `--parameters` accepted at version 0, where it would be dropped without a word.
    #[test]
    fn mock_refuses_parameters_at_bmf_v0() {
        assert!(matches!(
            mock(&["--bmf-version", "0", "--parameters", r#"{"size": 64}"#]),
            Err(MockError::ParametersAtV0)
        ));
    }

    /// Kills: `--bmf-version 0` ignored, and a version 0 triple whose bounds are not 10% either
    /// side of its value.
    #[test]
    fn mock_bmf_v0_writes_the_metric_triple() {
        let json = generate(&["--seed", "6", "--count", "2", "--bmf-version", "0"]);
        assert_eq!(read(&json, BmfVersion::V1), None);
        let results = read(&json, BmfVersion::V0).expect("Failed to read BMF v0");
        let metrics = metrics(&results);
        assert_eq!(
            names(&metrics),
            [
                "bencher::mock_0 {} latency lower_value",
                "bencher::mock_0 {} latency upper_value",
                "bencher::mock_0 {} latency value",
                "bencher::mock_1 {} latency lower_value",
                "bencher::mock_1 {} latency upper_value",
                "bencher::mock_1 {} latency value",
            ]
        );
        for triple in metrics.chunks(3) {
            let [(_, lower), (_, upper), (_, value)] = triple else {
                panic!("Expected a metric triple: {triple:?}");
            };
            assert!((lower - value * 0.9).abs() < 1e-9, "{triple:?}");
            assert!((upper - value * 1.1).abs() < 1e-9, "{triple:?}");
        }
    }

    /// Kills: `--seed` ignored, and version 1 values drawn in another order than version 0's,
    /// which would change what a seed means.
    #[test]
    fn mock_seed_draws_the_same_values_at_each_version() {
        let values = |args: &[&str], bmf_version| {
            let json = generate(args);
            let results = read(&json, bmf_version).expect("Failed to read results");
            metrics(&results)
                .into_iter()
                .filter(|(name, _)| name.ends_with(" value"))
                .map(|(_, value)| value)
                .collect::<Vec<_>>()
        };
        let v1 = values(&["--seed", "42"], BmfVersion::V1);
        assert_eq!(v1, values(&["--seed", "42"], BmfVersion::V1));
        assert_eq!(
            v1,
            values(&["--seed", "42", "--bmf-version", "0"], BmfVersion::V0)
        );
    }

    /// Kills: `--fail` dropped on its way from the parser.
    #[test]
    fn mock_fail_fails_at_each_version() {
        for args in [&["--fail"][..], &["--fail", "--bmf-version", "0"]] {
            let mock = mock(args).expect("Failed to build mock");
            assert!(
                matches!(mock.exec_inner(), Err(MockError::MockFailure)),
                "{args:?}"
            );
        }
    }
}
