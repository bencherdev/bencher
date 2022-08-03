use std::{
    convert::TryFrom,
    str::FromStr,
};

use async_trait::async_trait;
use bencher_json::JsonNewReport;
use chrono::Utc;
use git2::Oid;
use uuid::Uuid;

use crate::{
    bencher::{
        locality::Locality,
        wide::Wide,
    },
    cli::run::CliRun,
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
    branch:    Uuid,
    hash:      Option<Oid>,
    testbed:   Option<Uuid>,
    adapter:   Adapter,
}

impl TryFrom<CliRun> for Run {
    type Error = BencherError;

    fn try_from(run: CliRun) -> Result<Self, Self::Error> {
        Ok(Self {
            locality:  Locality::try_from(run.locality)?,
            benchmark: Benchmark::try_from(run.command)?,
            branch:    Uuid::from_str(&run.branch)?,
            hash:      if let Some(hash) = run.hash {
                Some(Oid::from_str(&hash)?)
            } else {
                None
            },
            testbed:   if let Some(testbed) = run.testbed {
                Some(Uuid::from_str(&testbed)?)
            } else {
                None
            },
            adapter:   map_adapter(run.adapter)?,
        })
    }
}

#[async_trait]
impl SubCmd for Run {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        let output = self.benchmark.run()?;
        let benchmarks = self.adapter.convert(&output)?;
        let report = JsonNewReport {
            branch: self.branch.clone(),
            hash: self.hash.clone().map(|hash| hash.to_string()),
            testbed: self.testbed.clone(),
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
