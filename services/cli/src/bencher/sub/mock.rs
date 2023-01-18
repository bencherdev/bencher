use std::collections::HashMap;

use async_trait::async_trait;
use bencher_adapter::{
    results::{adapter_metrics::AdapterMetrics, LATENCY_RESOURCE_ID},
    AdapterResults,
};
use bencher_json::JsonMetric;
use literally::hmap;
use rand::Rng;

use crate::{cli::mock::CliMock, cli_println, CliError};

use super::SubCmd;

const DEFAULT_COUNT: usize = 5;

#[derive(Debug, Clone)]
pub struct Mock {
    pub count: Option<usize>,
}

impl From<CliMock> for Mock {
    fn from(mock: CliMock) -> Self {
        let CliMock { count } = mock;
        Self { count }
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
            let value = 1_000.0 * (c + 1) as f64 * rng.gen::<f64>();
            let variance = value * rng.gen::<f64>() * 0.1;
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

        Ok(())
    }
}
