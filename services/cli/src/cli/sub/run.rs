use std::convert::TryFrom;

use async_trait::async_trait;
use reports::Report;

use crate::cli::adapter::map_adapter;
use crate::cli::adapter::Adapter;
use crate::cli::benchmark::Benchmark;
use crate::cli::clap::CliRun;
use crate::cli::sub::SubCmd;
use crate::cli::wide::Backend;
use crate::cli::wide::Wide;
use crate::BencherError;

#[derive(Debug)]
pub struct Run {
    benchmark: Benchmark,
    adapter: Adapter,
    project: Option<String>,
    testbed: Option<String>,
}

impl TryFrom<CliRun> for Run {
    type Error = BencherError;

    fn try_from(run: CliRun) -> Result<Self, Self::Error> {
        Ok(Self {
            benchmark: Benchmark::try_from((run.shell, run.cmd))?,
            adapter: map_adapter(run.adapter)?,
            project: run.project,
            testbed: run.testbed,
        })
    }
}

#[async_trait]
impl SubCmd for Run {
    async fn exec(&self, wide: &Wide) -> Result<(), BencherError> {
        let output = self.benchmark.run()?;
        let metrics = self.adapter.convert(output)?;
        if let Wide::Backend(backend) = wide {
            let report = Report::new(
                backend.email.to_string(),
                backend.token.clone(),
                self.project.clone(),
                self.testbed.clone(),
                metrics,
            );
            self.send(backend, &report).await
        } else {
            let report = Report::new(
                "".into(),
                "".into(),
                self.project.clone(),
                self.testbed.clone(),
                metrics,
            );
            Ok(println!("{}", serde_json::to_string(&report)?))
        }
    }
}

impl Run {
    pub async fn send(&self, backend: &Backend, report: &Report) -> Result<(), BencherError> {
        let client = reqwest::Client::new();
        let url = backend.url.join("/v0/reports")?.to_string();
        let res = client.put(&url).json(report).send().await?;
        println!("{res:?}");
        Ok(())
    }
}
