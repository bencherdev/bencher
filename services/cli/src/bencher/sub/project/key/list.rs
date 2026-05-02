use bencher_json::{ProjectResourceId, ResourceName};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::key::CliProjectKeyList,
};

#[derive(Debug, Clone)]
pub struct List {
    pub project: ProjectResourceId,
    pub name: Option<ResourceName>,
    pub search: Option<String>,
    pub revoked: bool,
    pub backend: AuthBackend,
}

impl TryFrom<CliProjectKeyList> for List {
    type Error = CliError;

    fn try_from(list: CliProjectKeyList) -> Result<Self, Self::Error> {
        let CliProjectKeyList {
            project,
            name,
            search,
            revoked,
            pagination: _,
            backend,
        } = list;
        Ok(Self {
            project,
            name,
            search,
            revoked,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for List {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                let mut request = client.proj_keys_get().project(self.project.clone());
                if let Some(name) = self.name.clone() {
                    request = request.name(name);
                }
                if let Some(search) = self.search.clone() {
                    request = request.search(search);
                }
                if self.revoked {
                    request = request.revoked(true);
                }
                request.send().await
            })
            .await?;
        Ok(())
    }
}
