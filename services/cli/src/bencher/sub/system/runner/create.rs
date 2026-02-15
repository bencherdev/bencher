use bencher_client::types::JsonNewRunner;
use bencher_json::{ResourceName, RunnerSlug};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::runner::CliRunnerCreate,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub name: ResourceName,
    pub slug: Option<RunnerSlug>,
    pub backend: AuthBackend,
}

impl TryFrom<CliRunnerCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliRunnerCreate) -> Result<Self, Self::Error> {
        let CliRunnerCreate {
            name,
            slug,
            backend,
        } = create;
        Ok(Self {
            name,
            slug,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewRunner {
    fn from(create: Create) -> Self {
        let Create { name, slug, .. } = create;
        Self {
            name: name.into(),
            slug: slug.map(Into::into),
        }
    }
}

impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move { client.runners_post().body(self.clone()).send().await })
            .await?;
        Ok(())
    }
}
