use std::collections::HashMap;

use async_trait::async_trait;
use bencher_adapter::{results::adapter_metrics::AdapterMetrics, AdapterResults};
use bencher_json::{project::metric_kind::LATENCY_SLUG, JsonMetric};
use literally::hmap;
use rand::Rng;

use crate::{cli::mock::CliMock, CliError};

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
    async fn exec(&self) -> Result<(), CliError> {
        let count = self.count.unwrap_or(DEFAULT_COUNT);
        let mut results = HashMap::with_capacity(count);
        let mut rng = rand::thread_rng();
        for c in 0..count {
            let value = 1_000.0 * (c + 1) as f64 * rng.gen::<f64>();
            let bound = (value * rng.gen::<f64>()).into();
            results.insert(
                format!("bencher::mock_{c}"),
                AdapterMetrics {
                    inner: hmap! {
                        LATENCY_SLUG => JsonMetric {
                             value: value.into(),
                             lower_bound: Some(bound),
                             upper_bound: Some(bound),
                        }
                    },
                },
            );
        }

        println!(
            "{}",
            serde_json::to_string_pretty(&AdapterResults::from(results))?
        );

        Ok(())
    }
}
