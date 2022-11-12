use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonPerfQuery, ResourceId};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{bencher::backend::Backend, cli::project::perf::CliPerf, CliError};

use crate::bencher::SubCmd;

#[derive(Debug, Clone)]
pub struct Perf {
    project: ResourceId,
    branches: Vec<Uuid>,
    testbeds: Vec<Uuid>,
    benchmarks: Vec<Uuid>,
    metric_kind: ResourceId,
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
    backend: Backend,
}

impl TryFrom<CliPerf> for Perf {
    type Error = CliError;

    fn try_from(perf: CliPerf) -> Result<Self, Self::Error> {
        let CliPerf {
            project,
            branches,
            testbeds,
            benchmarks,
            kind,
            start_time,
            end_time,
            backend,
        } = perf;
        Ok(Self {
            project,
            branches,
            testbeds,
            benchmarks,
            metric_kind: kind,
            start_time,
            end_time,
            backend: backend.try_into()?,
        })
    }
}

impl From<Perf> for JsonPerfQuery {
    fn from(perf: Perf) -> Self {
        let Perf {
            project: _,
            branches,
            testbeds,
            benchmarks,
            metric_kind,
            start_time,
            end_time,
            backend: _,
        } = perf;
        Self {
            branches,
            testbeds,
            benchmarks,
            metric_kind,
            start_time,
            end_time,
        }
    }
}

#[async_trait]
impl SubCmd for Perf {
    async fn exec(&self) -> Result<(), CliError> {
        let perf: JsonPerfQuery = self.clone().into();
        self.backend
            .put(&format!("/v0/projects/{}/branches", self.project), &perf)
            .await?;
        Ok(())
    }
}
