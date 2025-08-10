use bencher_json::{ProjectResourceId, ReportUuid};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::report::CliReportDelete,
};

#[derive(Debug)]
pub struct Delete {
    pub project: ProjectResourceId,
    pub report: ReportUuid,
    pub backend: AuthBackend,
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

impl SubCmd for Delete {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_report_delete()
                    .project(self.project.clone())
                    .report(self.report)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
