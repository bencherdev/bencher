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
    cli::run::{
        CliRun,
        CliRunAdapter,
    },
    BencherError,
};

mod adapter;
mod benchmark;

use adapter::Adapter;
use benchmark::Benchmark;

use super::SubCmd;

const REPORTS_PATH: &str = "/v0/reports";
const BENCHER_BRANCH: &str = "BENCHER_BRANCH";
const BENCHER_TESTBED: &str = "BENCHER_TESTBED";

#[derive(Debug)]
pub struct Run {
    locality:  Locality,
    benchmark: Benchmark,
    branch:    Uuid,
    hash:      Option<Oid>,
    testbed:   Uuid,
    adapter:   Adapter,
}

impl TryFrom<CliRun> for Run {
    type Error = BencherError;

    fn try_from(run: CliRun) -> Result<Self, Self::Error> {
        Ok(Self {
            locality:  Locality::try_from(run.locality)?,
            benchmark: Benchmark::try_from(run.command)?,
            branch:    unwrap_branch(run.branch)?,
            hash:      map_hash(run.hash)?,
            testbed:   unwrap_testbed(run.testbed)?,
            adapter:   unwrap_adapter(run.adapter),
        })
    }
}

fn unwrap_branch(branch: Option<Uuid>) -> Result<Uuid, BencherError> {
    Ok(if let Some(branch) = branch {
        branch
    } else if let Ok(branch) = std::env::var(BENCHER_BRANCH) {
        Uuid::from_str(&branch)?
    } else {
        return Err(BencherError::BranchNotFound);
    })
}

fn map_hash(hash: Option<String>) -> Result<Option<Oid>, BencherError> {
    Ok(if let Some(hash) = hash {
        Some(Oid::from_str(&hash)?)
    } else {
        None
    })
}

fn unwrap_testbed(testbed: Option<Uuid>) -> Result<Uuid, BencherError> {
    Ok(if let Some(testbed) = testbed {
        testbed
    } else if let Ok(testbed) = std::env::var(BENCHER_TESTBED) {
        Uuid::from_str(&testbed)?
    } else {
        return Err(BencherError::TestbedNotFound);
    })
}

fn unwrap_adapter(adapter: Option<CliRunAdapter>) -> Adapter {
    adapter.map(Into::into).unwrap_or_default()
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
