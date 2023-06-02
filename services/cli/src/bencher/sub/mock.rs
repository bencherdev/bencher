use std::collections::HashMap;

use async_trait::async_trait;
use bencher_adapter::{
    results::{adapter_metrics::AdapterMetrics, LATENCY_RESOURCE_ID},
    AdapterResults,
};
use bencher_json::JsonMetric;
use literally::hmap;
use rand::{distributions::Uniform, prelude::Distribution, Rng};

use crate::{cli::mock::CliMock, cli_println, CliError};

use super::SubCmd;

const DEFAULT_COUNT: usize = 5;

#[derive(Debug, Clone)]
pub struct Mock {
    pub count: Option<usize>,
    pub fail: bool,
    pub flaky: bool,
}

impl From<CliMock> for Mock {
    fn from(mock: CliMock) -> Self {
        let CliMock { count, fail, flaky } = mock;
        Self { count, fail, flaky }
    }
}

#[async_trait]
impl SubCmd for Mock {
    #[allow(
        clippy::arithmetic_side_effects,
        clippy::cast_precision_loss,
        clippy::float_arithmetic
    )]
    async fn exec(&self) -> Result<(), CliError> {
        let count = self.count.unwrap_or(DEFAULT_COUNT);
        let mut results = HashMap::with_capacity(count);
        let mut rng = rand::thread_rng();
        for c in 0..count {
            let offset = 10.0 * c as f64;
            let low = 0.0 + offset;
            let high = 10.0 + offset;
            let uniform = Uniform::new(low, high);
            let value: f64 = uniform.sample(&mut rng);
            let variance = value * 0.1;
            results.insert(
                format!("bencher::mock_{c}").as_str().parse()?,
                AdapterMetrics {
                    inner: hmap! {
                        LATENCY_RESOURCE_ID.clone() => JsonMetric {
                             value: value.into(),
                             lower_bound: Some((value - variance).into()),
                             upper_bound: Some((value + variance).into()),
                        }
                    },
                },
            );
        }

        cli_println!(
            "{}",
            serde_json::to_string_pretty(&AdapterResults::from(results))?
        );

        if self.fail || (self.flaky && rng.gen::<bool>()) {
            return Err(CliError::MockFailure);
        }

        Ok(())
    }
}
