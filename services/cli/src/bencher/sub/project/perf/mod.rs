use std::future::Future;
use std::pin::Pin;

use bencher_json::{
    BenchmarkUuid, BranchUuid, DateTime, HeadUuid, JsonPerf, JsonPerfQuery, MeasureUuid,
    ProjectResourceId, TestbedUuid,
};
use tabled::Table;

use crate::parser::ElidedOption;
use crate::{CliError, bencher::backend::PubBackend, cli_println, parser::project::perf::CliPerf};

use crate::bencher::SubCmd;

mod table_style;

use table_style::TableStyle;

#[derive(Debug, Clone)]
#[expect(clippy::option_option)]
pub struct Perf {
    project: ProjectResourceId,
    branches: Vec<BranchUuid>,
    heads: Vec<Option<HeadUuid>>,
    testbeds: Vec<TestbedUuid>,
    benchmarks: Vec<BenchmarkUuid>,
    measures: Vec<MeasureUuid>,
    start_time: Option<DateTime>,
    end_time: Option<DateTime>,
    table: Option<Option<TableStyle>>,
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
            benchmarks,
            measures,
            start_time,
            end_time,
            table,
            backend,
        } = perf;
        let backend = PubBackend::try_from(backend)?.log(table.is_none());
        Ok(Self {
            project,
            branches,
            heads: heads.into_iter().map(ElidedOption::into).collect(),
            testbeds,
            benchmarks,
            measures,
            start_time,
            end_time,
            table: table.map(|t| t.map(Into::into)),
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
            benchmarks,
            measures,
            start_time,
            end_time,
        }
    }
}

impl SubCmd for Perf {
    async fn exec(&self) -> Result<(), CliError> {
        let sender = perf_sender(self.project.clone(), self.clone());
        if let Some(table_style) = self.table {
            let json_perf: JsonPerf = self.backend.send_with(sender).await?;
            let mut perf_table: Table = json_perf.into();
            if let Some(table_style) = table_style {
                table_style.stylize(&mut perf_table);
            }
            cli_println!("{perf_table}");
        } else {
            self.backend.send(sender).await?;
        }
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
