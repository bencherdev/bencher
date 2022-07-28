use std::{
    convert::TryFrom,
    str::FromStr,
};

use async_trait::async_trait;
use bencher_json::JsonNewReport;
use chrono::Utc;
use uuid::Uuid;

use crate::{
    bencher::{
        locality::Locality,
        wide::Wide,
    },
    cli::CliRun,
    BencherError,
};

mod adapter;
mod benchmark;

use adapter::{
    map_adapter,
    Adapter,
};
use benchmark::Benchmark;

use super::SubCmd;

const REPORTS_PATH: &str = "/v0/reports";

#[derive(Debug)]
pub struct Run {
    locality:  Locality,
    benchmark: Benchmark,
    adapter:   Adapter,
    project:   Option<String>,
    testbed:   Option<String>,
}

impl TryFrom<CliRun> for Run {
    type Error = BencherError;

    fn try_from(run: CliRun) -> Result<Self, Self::Error> {
        Ok(Self {
            locality:  Locality::try_from(run.locality)?,
            benchmark: Benchmark::try_from(run.command)?,
            adapter:   map_adapter(run.adapter)?,
            project:   run.project,
            testbed:   run.testbed,
        })
    }
}

#[async_trait]
impl SubCmd for Run {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        let output = self.benchmark.run()?;
        let benchmarks = self.adapter.convert(&output)?;
        let testbed = if let Some(testbed) = &self.testbed {
            if let Ok(uuid) = Uuid::from_str(testbed) {
                Some(uuid)
            } else {
                None
            }
        } else {
            None
        };
        let report = JsonNewReport {
            project: self.project.clone(),
            testbed,
            adapter: self.adapter.into(),
            start_time: output.start,
            end_time: Utc::now(),
            benchmarks,
        };
        match &self.locality {
            Locality::Local => Ok(println!("{}", serde_json::to_string(&report)?)),
            Locality::Backend(backend) => {
                backend.post(REPORTS_PATH, &report).await?;
                Ok(())
            },
        }
    }
}
