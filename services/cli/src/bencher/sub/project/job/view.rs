use bencher_json::{JobUuid, ProjectResourceId};

use crate::{
    CliError,
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::job::CliJobView,
};

#[derive(Debug)]
pub struct View {
    pub project: ProjectResourceId,
    pub job: JobUuid,
    pub backend: PubBackend,
}

impl TryFrom<CliJobView> for View {
    type Error = CliError;

    fn try_from(view: CliJobView) -> Result<Self, Self::Error> {
        let CliJobView {
            project,
            job,
            backend,
        } = view;
        Ok(Self {
            project,
            job,
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
                    .proj_job_get()
                    .project(self.project.clone())
                    .job(self.job)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
