use std::convert::TryFrom;

use reports::{Metrics, Report, Testbed};

use crate::cli::adapter::Adapter;
use crate::cli::benchmark::Benchmark;
use crate::cli::benchmark::BenchmarkOutput;
use crate::cli::clap::CliRun;
use crate::cli::wide::Wide;
use crate::BencherError;

#[derive(Debug)]
pub struct Run {
    benchmark: Benchmark,
    adapter: Adapter,
    project: Option<String>,
    testbed: Testbed,
}

impl TryFrom<CliRun> for Run {
    type Error = BencherError;

    fn try_from(run: CliRun) -> Result<Self, Self::Error> {
        Ok(Self {
            benchmark: Benchmark::try_from(run.benchmark)?,
            adapter: Adapter::from(run.adapter),
            project: run.project,
            testbed: run.testbed.into(),
        })
    }
}

impl Run {
    pub async fn run(&self, wide: &Wide) -> Result<(), BencherError> {
        let output = self.benchmark.run()?;
        let metrics = self.adapter.convert(output)?;
        let report = Report::new(
            wide.email.to_string(),
            wide.token.clone(),
            self.project.clone(),
            self.testbed.clone(),
            metrics,
        );
        wide.send(report).await
    }
}
