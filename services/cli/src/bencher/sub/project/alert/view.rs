use bencher_json::{AlertUuid, ProjectResourceId};

use crate::{
    CliError,
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::alert::CliAlertView,
};

#[derive(Debug)]
pub struct View {
    pub project: ProjectResourceId,
    pub alert: AlertUuid,
    pub backend: PubBackend,
}

impl TryFrom<CliAlertView> for View {
    type Error = CliError;

    fn try_from(view: CliAlertView) -> Result<Self, Self::Error> {
        let CliAlertView {
            project,
            alert,
            backend,
        } = view;
        Ok(Self {
            project,
            alert,
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
                    .proj_alert_get()
                    .project(self.project.clone())
                    .alert(self.alert)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
