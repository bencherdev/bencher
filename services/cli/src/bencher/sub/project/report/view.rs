use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonReport, ResourceId};
use uuid::Uuid;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::report::CliReportView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub report: Uuid,
    pub backend: Backend,
}

impl TryFrom<CliReportView> for View {
    type Error = CliError;

    fn try_from(view: CliReportView) -> Result<Self, Self::Error> {
        let CliReportView {
            project,
            report,
            backend,
        } = view;
        Ok(Self {
            project,
            report,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonReport = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_report_get()
                        .project(self.project.clone())
                        .report_uuid(self.report)
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
