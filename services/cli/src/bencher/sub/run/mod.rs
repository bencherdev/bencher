use std::{convert::TryFrom, str::FromStr};

use async_trait::async_trait;
use bencher_json::{report::new::JsonBenchmarks, JsonNewReport, JsonReport};
use chrono::Utc;
use git2::Oid;
use uuid::Uuid;

use crate::{
    bencher::{locality::Locality, wide::Wide},
    cli::run::{CliRun, CliRunAdapter, CliRunFold},
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
    perf: Perf,
    branch: Uuid,
    hash: Option<Oid>,
    testbed: Uuid,
    adapter: Adapter,
    iter: usize,
    fold: Option<Fold>,
    err: bool,
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
            err,
        } = run;
        Ok(Self {
            locality: locality.try_into()?,
            perf: command.try_into()?,
            branch: unwrap_branch(branch)?,
            hash: map_hash(hash)?,
            testbed: unwrap_testbed(testbed)?,
            adapter: unwrap_adapter(adapter),
            iter: iter.unwrap_or(1),
            fold: fold.map(Into::into),
            err,
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
        let start_time = Utc::now();

        let mut benchmarks = Vec::with_capacity(self.iter);
        for _ in 0..self.iter {
            let output = self.perf.run()?;
            let benchmark_perf = self.adapter.convert(&output)?;
            benchmarks.push(benchmark_perf);
        }
        let mut benchmarks = benchmarks.into();

        if let Some(ref fold) = self.fold {
            benchmarks = fold.fold(benchmarks)
        }

        let report = JsonNewReport {
            branch: self.branch,
            hash: self.hash.map(|hash| hash.to_string()),
            testbed: self.testbed,
            adapter: self.adapter.into(),
            start_time,
            end_time: Utc::now(),
            benchmarks,
        };

        match &self.locality {
            Locality::Local => println!("{}", serde_json::to_string_pretty(&report)?),
            Locality::Backend(backend) => {
                let value = backend.post(REPORTS_PATH, &report).await?;
                if self.err {
                    let json_report: JsonReport = serde_json::from_value(value)?;
                    if !json_report.alerts.is_empty() {
                        return Err(BencherError::Alerts);
                    }
                }
            },
        }

        Ok(())
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

impl Fold {
    fn fold(&self, benchmarks: JsonBenchmarks) -> JsonBenchmarks {
        if benchmarks.inner.is_empty() {
            return benchmarks;
        }

        match self {
            Self::Min => benchmarks.min(),
            Self::Max => benchmarks.max(),
            Self::Mean => benchmarks.mean(),
            Self::Median => benchmarks.median(),
        }
    }
}
