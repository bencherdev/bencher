use bencher_json::{ChangepointUuid, ProjectResourceId};

use crate::{
    CliError,
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::changepoint::CliChangepointView,
};

#[derive(Debug)]
pub struct View {
    pub project: ProjectResourceId,
    pub changepoint: ChangepointUuid,
    pub backend: PubBackend,
}

impl TryFrom<CliChangepointView> for View {
    type Error = CliError;

    fn try_from(view: CliChangepointView) -> Result<Self, Self::Error> {
        let CliChangepointView {
            project,
            changepoint,
            backend,
        } = view;
        Ok(Self {
            project,
            changepoint,
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
                    .proj_changepoint_get()
                    .project(self.project.clone())
                    .changepoint(self.changepoint)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
