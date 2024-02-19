use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::server::CliConfigView,
    CliError,
};

#[derive(Debug, Clone)]
pub struct View {
    pub backend: AuthBackend,
}

impl TryFrom<CliConfigView> for View {
    type Error = CliError;

    fn try_from(view: CliConfigView) -> Result<Self, Self::Error> {
        let CliConfigView { backend } = view;
        Ok(Self {
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move { client.server_config_get().send().await })
            .await?;
        Ok(())
    }
}
