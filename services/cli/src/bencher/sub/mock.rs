use std::collections::HashMap;

use bencher_adapter::{AdapterResults, results::adapter_metrics::AdapterMetrics};
use bencher_json::{JsonNewMetric, NameId};
use rand::{
    Rng as _,
    distr::{Distribution as _, Uniform},
};

use crate::{CliError, cli_println, parser::mock::CliMock};

use super::SubCmd;

const DEFAULT_COUNT: usize = 5;

#[derive(Debug, Clone)]
pub struct Mock {
    pub count: Option<usize>,
    pub measures: Vec<NameId>,
    pub pow: Option<i32>,
    pub fail: bool,
    pub flaky: bool,
}

#[allow(clippy::absolute_paths)]
#[derive(thiserror::Error, Debug)]
pub enum MockError {
    #[error("Failed to generate uniform distribution: {0}")]
    BadDistribution(rand::distr::uniform::Error),

    #[error("Failed to parse benchmark name: {0}")]
    ParseBenchmarkName(bencher_json::ValidError),

    #[error("Failed to serialize mock results: {0}")]
    SerializeResults(serde_json::Error),

    #[error("Mock failure")]
    MockFailure,
}

impl From<CliMock> for Mock {
    fn from(mock: CliMock) -> Self {
        let CliMock {
            count,
            measure,
            pow,
            fail,
            flaky,
        } = mock;
        Self {
            count,
            measures: measure,
            pow,
            fail,
            flaky,
        }
    }
}

impl SubCmd for Mock {
    async fn exec(&self) -> Result<(), CliError> {
        self.exec_inner().map_err(Into::into)
    }
}

impl Mock {
    fn exec_inner(&self) -> Result<(), MockError> {
        let adapter_results = self.generate_results()?;

        cli_println!(
            "{}",
            serde_json::to_string_pretty(&adapter_results).map_err(MockError::SerializeResults)?
        );

        if self.fail || (self.flaky && rand::rng().random::<bool>()) {
            Err(MockError::MockFailure)
        } else {
            Ok(())
        }
    }

    #[allow(clippy::cast_precision_loss, clippy::similar_names)]
    fn generate_results(&self) -> Result<AdapterResults, MockError> {
        let count = self.count.unwrap_or(DEFAULT_COUNT);
        let pow = self.pow.unwrap_or(1);
        let ten_pow = 10.0f64.powi(pow);
        let mut results = HashMap::with_capacity(count);
        let mut rng = rand::rng();
        for c in 0..count {
            let mut measures_map = HashMap::with_capacity(self.measures.len());
            for measure in self.measures.clone() {
                let low = ten_pow * c as f64;
                let high = ten_pow * (c + 1) as f64;
                let uniform = Uniform::new(low, high).map_err(MockError::BadDistribution)?;
                let value: f64 = uniform.sample(&mut rng);
                let variance = value * 0.1;
                let metric = JsonNewMetric {
                    value: value.into(),
                    lower_value: Some((value - variance).into()),
                    upper_value: Some((value + variance).into()),
                };
                measures_map.insert(measure.clone(), metric);
            }
            results.insert(
                format!("bencher::mock_{c}")
                    .as_str()
                    .parse()
                    .map_err(MockError::ParseBenchmarkName)?,
                AdapterMetrics {
                    inner: measures_map,
                },
            );
        }

        Ok(AdapterResults::from(results))
    }
}
