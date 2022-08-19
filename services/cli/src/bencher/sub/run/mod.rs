use std::{
    convert::TryFrom,
    str::FromStr,
};

use async_trait::async_trait;
use bencher_json::{
    report::JsonNewBenchmarks,
    JsonNewReport,
};
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
        CliRunFold,
    },
    BencherError,
};

mod adapter;
mod perf;

use adapter::Adapter;
use perf::Perf;

use super::SubCmd;

const REPORTS_PATH: &str = "/v0/reports";
const BENCHER_BRANCH: &str = "BENCHER_BRANCH";
const BENCHER_TESTBED: &str = "BENCHER_TESTBED";

#[derive(Debug)]
pub struct Run {
    locality: Locality,
    perf:     Perf,
    branch:   Uuid,
    hash:     Option<Oid>,
    testbed:  Uuid,
    adapter:  Adapter,
    iter:     usize,
    fold:     Option<Fold>,
}

impl TryFrom<CliRun> for Run {
    type Error = BencherError;

    fn try_from(run: CliRun) -> Result<Self, Self::Error> {
        let CliRun {
            locality,
            command,
            branch,
            hash,
            testbed,
            adapter,
            iter,
            fold,
        } = run;
        Ok(Self {
            locality: Locality::try_from(locality)?,
            perf:     Perf::try_from(command)?,
            branch:   unwrap_branch(branch)?,
            hash:     map_hash(hash)?,
            testbed:  unwrap_testbed(testbed)?,
            adapter:  unwrap_adapter(adapter),
            iter:     iter.unwrap_or(1),
            fold:     fold.map(Into::into),
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
        let output = self.perf.run()?;
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
            Locality::Local => Ok(println!("{}", serde_json::to_string_pretty(&report)?)),
            Locality::Backend(backend) => {
                backend.post(REPORTS_PATH, &report).await?;
                Ok(())
            },
        }
    }
}

#[derive(Debug)]
enum Fold {
    Min,
    Max,
    Mean,
    Median,
}

impl From<CliRunFold> for Fold {
    fn from(fold: CliRunFold) -> Self {
        match fold {
            CliRunFold::Min => Self::Min,
            CliRunFold::Max => Self::Max,
            CliRunFold::Mean => Self::Mean,
            CliRunFold::Median => Self::Median,
        }
    }
}
