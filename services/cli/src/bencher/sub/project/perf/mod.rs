use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{
    BenchmarkUuid, BranchUuid, DateTime, JsonPerf, JsonPerfQuery, MetricKindUuid, ResourceId,
    TestbedUuid,
};
use tabled::Table;

use crate::{bencher::backend::Backend, cli_println, parser::project::perf::CliPerf, CliError};

use crate::bencher::SubCmd;

mod table_style;

use table_style::TableStyle;

#[derive(Debug, Clone)]
#[allow(clippy::option_option)]
pub struct Perf {
    project: ResourceId,
    metric_kinds: Vec<MetricKindUuid>,
    branches: Vec<BranchUuid>,
    testbeds: Vec<TestbedUuid>,
    benchmarks: Vec<BenchmarkUuid>,
    start_time: Option<DateTime>,
    end_time: Option<DateTime>,
    table: Option<Option<TableStyle>>,
    backend: Backend,
}

impl TryFrom<CliPerf> for Perf {
    type Error = CliError;

    fn try_from(perf: CliPerf) -> Result<Self, Self::Error> {
        let CliPerf {
            project,
            metric_kinds,
            branches,
            testbeds,
            benchmarks,
            start_time,
            end_time,
            table,
            backend,
        } = perf;
        Ok(Self {
            project,
            metric_kinds,
            branches,
            testbeds,
            benchmarks,
            start_time,
            end_time,
            table: table.map(|t| t.map(Into::into)),
            backend: backend.try_into()?,
        })
    }
}

impl From<Perf> for JsonPerfQuery {
    fn from(perf: Perf) -> Self {
        let Perf {
            metric_kinds,
            branches,
            testbeds,
            benchmarks,
            start_time,
            end_time,
            ..
        } = perf;
        Self {
            metric_kinds,
            branches,
            testbeds,
            benchmarks,
            start_time,
            end_time,
        }
    }
}

#[async_trait]
impl SubCmd for Perf {
    async fn exec(&self) -> Result<(), CliError> {
        let json_perf_query: &JsonPerfQuery = &self.clone().into();
        let json_perf: JsonPerf = self
            .backend
            .send_with(
                |client| async move {
                    let mut client = client
                        .proj_perf_get()
                        .project(self.project.clone())
                        .metric_kinds(json_perf_query.metric_kinds())
                        .branches(json_perf_query.branches())
                        .testbeds(json_perf_query.testbeds())
                        .benchmarks(json_perf_query.benchmarks());

                    if let Some(start_time) = json_perf_query.start_time() {
                        client = client.start_time(start_time);
                    }
                    if let Some(end_time) = json_perf_query.end_time() {
                        client = client.end_time(end_time);
                    }

                    client.send().await
                },
                true,
            )
            .await?;

        if let Some(table_style) = self.table {
            let mut perf_table: Table = json_perf.into();
            if let Some(table_style) = table_style {
                table_style.stylize(&mut perf_table);
            }
            cli_println!("{perf_table}");
        }
        Ok(())
    }
}
