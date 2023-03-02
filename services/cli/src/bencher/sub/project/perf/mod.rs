use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::project::perf::JsonPerfQueryParams;
use bencher_json::{JsonPerfQuery, ResourceId};
use chrono::serde::ts_milliseconds_option::deserialize as from_milli_ts;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{bencher::backend::Backend, cli::project::perf::CliPerf, CliError};

use crate::bencher::SubCmd;

#[derive(Debug, Clone)]
pub struct Perf {
    project: ResourceId,
    metric_kind: ResourceId,
    branches: Vec<Uuid>,
    testbeds: Vec<Uuid>,
    benchmarks: Vec<Uuid>,
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
    backend: Backend,
}

impl TryFrom<CliPerf> for Perf {
    type Error = CliError;

    fn try_from(perf: CliPerf) -> Result<Self, Self::Error> {
        let CliPerf {
            project,
            metric_kind,
            branches,
            testbeds,
            benchmarks,
            start_time,
            end_time,
            backend,
        } = perf;
        Ok(Self {
            project,
            metric_kind,
            branches,
            testbeds,
            benchmarks,
            start_time: from_milli_ts(serde_json::json!(start_time))?,
            end_time: from_milli_ts(serde_json::json!(end_time))?,
            backend: backend.try_into()?,
        })
    }
}

impl From<Perf> for JsonPerfQuery {
    fn from(perf: Perf) -> Self {
        let Perf {
            metric_kind,
            branches,
            testbeds,
            benchmarks,
            start_time,
            end_time,
            ..
        } = perf;
        Self {
            metric_kind,
            branches,
            testbeds,
            benchmarks,
            start_time,
            end_time,
        }
    }
}

#[async_trait]
impl SubCmd for Perf {
    async fn exec(&self) -> Result<(), CliError> {
        let perf: JsonPerfQueryParams = JsonPerfQuery::from(self.clone()).try_into()?;
        self.backend
            .get_query(&format!("/v0/projects/{}/perf", self.project), &perf)
            .await?;
        Ok(())
    }
}
