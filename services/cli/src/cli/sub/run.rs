use std::convert::TryFrom;

use async_trait::async_trait;
use reports::{Report, Testbed};

use crate::cli::adapter::Adapter;
use crate::cli::benchmark::Benchmark;
use crate::cli::clap::CliRun;
use crate::cli::sub::SubCmd;
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
            benchmark: Benchmark::try_from((run.shell, run.cmd))?,
            adapter: Adapter::from(run.adapter),
            project: run.project,
            testbed: run.testbed.into(),
        })
    }
}

#[async_trait]
impl SubCmd for Run {
    async fn run(&self, wide: &Wide) -> Result<(), BencherError> {
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
