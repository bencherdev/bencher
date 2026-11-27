use std::future::Future;
use std::pin::Pin;

#[cfg(feature = "plus")]
use bencher_json::SpecUuid;
use bencher_json::{
    BenchmarkUuid, BranchUuid, DateTime, HeadUuid, JsonPerfQuery, MeasureUuid, ProjectResourceId,
    TestbedUuid,
};

use crate::parser::ElidedOption;
use crate::{CliError, bencher::backend::PubBackend, parser::project::perf::CliPerf};

use crate::bencher::SubCmd;

#[derive(Debug, Clone)]
pub struct Perf {
    project: ProjectResourceId,
    branches: Vec<BranchUuid>,
    heads: Vec<Option<HeadUuid>>,
    testbeds: Vec<TestbedUuid>,
    #[cfg(feature = "plus")]
    specs: Vec<Option<SpecUuid>>,
    benchmarks: Vec<BenchmarkUuid>,
    measures: Vec<MeasureUuid>,
    start_time: Option<DateTime>,
    end_time: Option<DateTime>,
    backend: PubBackend,
}

impl TryFrom<CliPerf> for Perf {
    type Error = CliError;

    fn try_from(perf: CliPerf) -> Result<Self, Self::Error> {
        let CliPerf {
            project,
            branches,
            heads,
            testbeds,
            #[cfg(feature = "plus")]
            specs,
            benchmarks,
            measures,
            start_time,
            end_time,
            backend,
        } = perf;
        let backend = PubBackend::try_from(backend)?;
        Ok(Self {
            project,
            branches,
            heads: heads.into_iter().map(ElidedOption::into).collect(),
            testbeds,
            #[cfg(feature = "plus")]
            specs: specs.into_iter().map(ElidedOption::into).collect(),
            benchmarks,
            measures,
            start_time,
            end_time,
            backend,
        })
    }
}

impl From<Perf> for JsonPerfQuery {
    fn from(perf: Perf) -> Self {
        let Perf {
            branches,
            heads,
            testbeds,
            #[cfg(feature = "plus")]
            specs,
            benchmarks,
            measures,
            start_time,
            end_time,
            ..
        } = perf;
        Self {
            branches,
            heads,
            testbeds,
            #[cfg(feature = "plus")]
            specs,
            benchmarks,
            // No filter, so every variant is queried.
            parameters: None,
            measures,
            start_time,
            end_time,
        }
    }
}

impl SubCmd for Perf {
    async fn exec(&self) -> Result<(), CliError> {
        let sender = perf_sender(self.project.clone(), self.clone());
        self.backend.send(sender).await?;
        Ok(())
    }
}

type PerfQueryResult = Pin<
    Box<
        dyn Future<
                Output = Result<
                    progenitor_client::ResponseValue<bencher_client::types::JsonPerf>,
                    bencher_client::Error<bencher_client::types::Error>,
                >,
            > + Send,
    >,
>;
fn perf_sender(
    project: ProjectResourceId,
    json_perf_query: impl Into<JsonPerfQuery>,
) -> Box<dyn Fn(bencher_client::Client) -> PerfQueryResult + Send> {
    let json_perf_query: JsonPerfQuery = json_perf_query.into();
    Box::new(move |client: bencher_client::Client| {
        let project = project.clone();
        let json_perf_query = json_perf_query.clone();
        Box::pin(async move {
            let mut client = client
                .proj_perf_get()
                .project(project.clone())
                .branches(json_perf_query.branches())
                .testbeds(json_perf_query.testbeds())
                .benchmarks(json_perf_query.benchmarks())
                .measures(json_perf_query.measures());

            if let Some(heads) = json_perf_query.heads() {
                client = client.heads(heads);
            }
            if let Some(specs) = json_perf_query.specs() {
                client = client.specs(specs);
            }

            if let Some(start_time) = json_perf_query.start_time() {
                client = client.start_time(start_time);
            }
            if let Some(end_time) = json_perf_query.end_time() {
                client = client.end_time(end_time);
            }

            client.send().await
        })
    })
}
