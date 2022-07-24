use std::{
    convert::TryFrom,
    str::FromStr,
};

use async_trait::async_trait;
use bencher_json::JsonReport;
use chrono::Utc;
use uuid::Uuid;

use crate::{
    cli::{
        adapter::{
            map_adapter,
            Adapter,
        },
        benchmark::Benchmark,
        clap::CliRun,
        locality::Locality,
        sub::SubCmd,
        wide::Wide,
    },
    BencherError,
};

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
        let metrics = self.adapter.convert(&output)?;
        let testbed = if let Some(testbed) = &self.testbed {
            if let Ok(uuid) = Uuid::from_str(testbed) {
                Some(uuid)
            } else {
                None
            }
        } else {
            None
        };
        let report = JsonReport::new(
            self.project.clone(),
            testbed,
            self.adapter.into(),
            output.start,
            Utc::now(),
            metrics,
        );
        match &self.locality {
            Locality::Local => Ok(println!("{}", serde_json::to_string(&report)?)),
            Locality::Backend(backend) => backend.post(REPORTS_PATH, &report).await,
        }
    }
}
