use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonPerf, JsonPerfQuery, ResourceId};
use chrono::{DateTime, Utc};
use tabled::Table;
use uuid::Uuid;

use crate::{bencher::backend::Backend, cli::project::perf::CliPerf, cli_println, CliError};

use crate::bencher::{map_timestamp_millis, SubCmd};

mod table_style;

use table_style::TableStyle;

#[derive(Debug, Clone)]
pub struct Perf {
    project: ResourceId,
    metric_kind: ResourceId,
    branches: Vec<Uuid>,
    testbeds: Vec<Uuid>,
    benchmarks: Vec<Uuid>,
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
    table: Option<Option<TableStyle>>,
    backend: Backend,
}

impl TryFrom<CliPerf> for Perf {
    type Error = CliError;

    fn try_from(perf: CliPerf) -> Result<Self, Self::Error> {
        let CliPerf {
            project,
            metric_kind,
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
            metric_kind,
            branches,
            testbeds,
            benchmarks,
            start_time: map_timestamp_millis(start_time)?,
            end_time: map_timestamp_millis(end_time)?,
            table: table.map(|t| t.map(Into::into)),
            backend: backend.try_into()?,
        })
    }
}

impl From<Perf> for JsonPerfQuery {
    fn from(perf: Perf) -> Self {
        let Perf {
            metric_kind,
            branches,
            testbeds,
            benchmarks,
            start_time,
            end_time,
            ..
        } = perf;
        Self {
            metric_kind,
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
        let perf: JsonPerfQuery = self.clone().into();
        let resp = self
            .backend
            .get_query(&format!("/v0/projects/{}/perf", self.project), &perf)
            .await?;
        if let Some(table_style) = self.table {
            let json_perf: JsonPerf = serde_json::from_value(resp)?;
            let mut perf_table: Table = json_perf.into();
            if let Some(table_style) = table_style {
                table_style.stylize(&mut perf_table);
            }
            cli_println!("{perf_table}");
        }
        Ok(())
    }
}
