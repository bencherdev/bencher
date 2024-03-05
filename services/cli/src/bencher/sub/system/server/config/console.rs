use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::system::server::CliConfigConsole,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Console {
    pub backend: PubBackend,
}

impl TryFrom<CliConfigConsole> for Console {
    type Error = CliError;

    fn try_from(console: CliConfigConsole) -> Result<Self, Self::Error> {
        let CliConfigConsole { backend } = console;
        Ok(Self {
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Console {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move { client.server_config_console_get().send().await })
            .await?;
        Ok(())
    }
}
