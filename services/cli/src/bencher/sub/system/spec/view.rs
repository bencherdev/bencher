use bencher_json::SpecResourceId;

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::spec::CliSpecView,
};

#[derive(Debug)]
pub struct View {
    pub spec: SpecResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliSpecView> for View {
    type Error = CliError;

    fn try_from(view: CliSpecView) -> Result<Self, Self::Error> {
        let CliSpecView { spec, backend } = view;
        Ok(Self {
            spec,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move { client.spec_get().spec(self.spec.clone()).send().await })
            .await?;
        Ok(())
    }
}
