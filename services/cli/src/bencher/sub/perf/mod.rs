use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::JsonPerfQuery;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    bencher::{backend::Backend, wide::Wide},
    cli::perf::CliPerf,
    BencherError,
};

pub mod kind;

use kind::Kind;

use super::SubCmd;

const PERF_PATH: &str = "/v0/perf";

#[derive(Debug)]
pub struct Perf {
    branches: Vec<Uuid>,
    testbeds: Vec<Uuid>,
    benchmarks: Vec<Uuid>,
    kind: Kind,
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
    backend: Backend,
}

impl TryFrom<CliPerf> for Perf {
    type Error = BencherError;

    fn try_from(perf: CliPerf) -> Result<Self, Self::Error> {
        let CliPerf {
            branches,
            testbeds,
            benchmarks,
            kind,
            start_time,
            end_time,
            backend,
        } = perf;
        Ok(Self {
            branches,
            testbeds,
            benchmarks,
            kind: kind.into(),
            start_time,
            end_time,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Perf {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        // TODO break this out into an Into impl
        let perf = JsonPerfQuery {
            branches: self.branches.clone(),
            testbeds: self.testbeds.clone(),
            benchmarks: self.benchmarks.clone(),
            kind: self.kind.into(),
            start_time: self.start_time,
            end_time: self.end_time,
        };
        self.backend.post(PERF_PATH, &perf).await?;
        Ok(())
    }
}
