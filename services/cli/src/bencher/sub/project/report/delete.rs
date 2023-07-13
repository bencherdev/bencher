use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonEmpty, ResourceId};
use uuid::Uuid;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::report::CliReportDelete,
    CliError,
};

#[derive(Debug)]
pub struct Delete {
    pub project: ResourceId,
    pub report: Uuid,
    pub backend: Backend,
}

impl TryFrom<CliReportDelete> for Delete {
    type Error = CliError;

    fn try_from(view: CliReportDelete) -> Result<Self, Self::Error> {
        let CliReportDelete {
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
impl SubCmd for Delete {
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonEmpty = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_report_delete()
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
