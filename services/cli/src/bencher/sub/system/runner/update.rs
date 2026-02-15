use bencher_client::types::JsonUpdateRunner;
use bencher_json::{ResourceName, RunnerResourceId, RunnerSlug};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::runner::CliRunnerUpdate,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub runner: RunnerResourceId,
    pub name: Option<ResourceName>,
    pub slug: Option<RunnerSlug>,
    pub archived: Option<bool>,
    pub backend: AuthBackend,
}

impl TryFrom<CliRunnerUpdate> for Update {
    type Error = CliError;

    fn try_from(update: CliRunnerUpdate) -> Result<Self, Self::Error> {
        let CliRunnerUpdate {
            runner,
            name,
            slug,
            archived,
            backend,
        } = update;
        Ok(Self {
            runner,
            name,
            slug,
            archived: archived.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateRunner {
    fn from(update: Update) -> Self {
        let Update {
            name,
            slug,
            archived,
            ..
        } = update;
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
            archived,
        }
    }
}

impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .runner_patch()
                    .runner(self.runner.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
