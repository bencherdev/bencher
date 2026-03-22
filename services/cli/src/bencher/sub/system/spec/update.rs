use bencher_client::types::JsonUpdateSpec;
use bencher_json::{ResourceName, SpecResourceId, SpecSlug};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::spec::CliSpecUpdate,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub spec: SpecResourceId,
    pub name: Option<ResourceName>,
    pub slug: Option<SpecSlug>,
    pub fallback: Option<bool>,
    pub archived: Option<bool>,
    pub backend: AuthBackend,
}

impl TryFrom<CliSpecUpdate> for Update {
    type Error = CliError;

    fn try_from(update: CliSpecUpdate) -> Result<Self, Self::Error> {
        let CliSpecUpdate {
            spec,
            name,
            slug,
            fallback,
            archived,
            backend,
        } = update;
        Ok(Self {
            spec,
            name,
            slug,
            fallback,
            archived: archived.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateSpec {
    fn from(update: Update) -> Self {
        let Update {
            name,
            slug,
            fallback,
            archived,
            ..
        } = update;
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
            fallback,
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
                    .spec_patch()
                    .spec(self.spec.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
