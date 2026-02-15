use bencher_json::SpecResourceId;

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::spec::CliSpecDelete,
};

#[derive(Debug)]
pub struct Delete {
    pub spec: SpecResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliSpecDelete> for Delete {
    type Error = CliError;

    fn try_from(delete: CliSpecDelete) -> Result<Self, Self::Error> {
        let CliSpecDelete { spec, backend } = delete;
        Ok(Self {
            spec,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Delete {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move { client.spec_delete().spec(self.spec.clone()).send().await })
            .await?;
        Ok(())
    }
}
