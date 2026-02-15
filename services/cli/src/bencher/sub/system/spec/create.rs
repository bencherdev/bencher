use bencher_client::types::JsonNewSpec;
use bencher_json::{Architecture, ResourceName, SpecSlug};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::spec::CliSpecCreate,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub name: ResourceName,
    pub slug: Option<SpecSlug>,
    pub architecture: Architecture,
    pub cpu: u32,
    pub memory: u64,
    pub disk: u64,
    pub network: bool,
    pub backend: AuthBackend,
}

impl TryFrom<CliSpecCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliSpecCreate) -> Result<Self, Self::Error> {
        let CliSpecCreate {
            name,
            slug,
            architecture,
            cpu,
            memory,
            disk,
            network,
            backend,
        } = create;
        Ok(Self {
            name,
            slug,
            architecture,
            cpu,
            memory,
            disk,
            network,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewSpec {
    fn from(create: Create) -> Self {
        let Create {
            name,
            slug,
            architecture,
            cpu,
            memory,
            disk,
            network,
            ..
        } = create;
        Self {
            name: name.into(),
            slug: slug.map(Into::into),
            architecture: architecture.into(),
            cpu: cpu.into(),
            memory: memory.into(),
            disk: disk.into(),
            network,
        }
    }
}

impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move { client.specs_post().body(self.clone()).send().await })
            .await?;
        Ok(())
    }
}
