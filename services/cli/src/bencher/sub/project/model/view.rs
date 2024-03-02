use bencher_json::{ModelUuid, ResourceId};

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::model::CliModelView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub model: ModelUuid,
    pub backend: PubBackend,
}

impl TryFrom<CliModelView> for View {
    type Error = CliError;

    fn try_from(view: CliModelView) -> Result<Self, Self::Error> {
        let CliModelView {
            project,
            model,
            backend,
        } = view;
        Ok(Self {
            project,
            model,
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
                    .proj_model_get()
                    .project(self.project.clone())
                    .model(self.model)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
