use std::convert::TryFrom;

use async_trait::async_trait;
use chrono::Utc;
use report::Report;

use crate::{
    cli::{
        adapter::{
            map_adapter,
            Adapter,
        },
        backend::Backend,
        benchmark::Benchmark,
        clap::CliRun,
        locality::Locality,
        sub::SubCmd,
        wide::Wide,
    },
    BencherError,
};

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
        let metrics = self.adapter.convert(&output)?;
        let report = Report::new(
            self.project.clone(),
            self.testbed.clone(),
            output.start,
            Utc::now(),
            metrics,
        );
        match &self.locality {
            Locality::Local => Ok(println!("{}", serde_json::to_string(&report)?)),
            Locality::Backend(backend) => self.send(backend, &report).await,
        }
    }
}

impl Run {
    pub async fn send(&self, backend: &Backend, report: &Report) -> Result<(), BencherError> {
        let client = reqwest::Client::new();
        let url = backend.url.join("/v0/reports")?.to_string();
        let res = client.post(&url).json(report).send().await?;
        println!("{res:?}");
        Ok(())
    }
}
