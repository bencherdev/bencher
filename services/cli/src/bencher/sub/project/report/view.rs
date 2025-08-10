use bencher_json::{ProjectResourceId, ReportUuid};

use crate::{
    CliError,
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::report::CliReportView,
};

#[derive(Debug)]
pub struct View {
    pub project: ProjectResourceId,
    pub report: ReportUuid,
    pub backend: PubBackend,
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

impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_report_get()
                    .project(self.project.clone())
                    .report(self.report)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
