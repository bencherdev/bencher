use std::convert::TryFrom;

use async_trait::async_trait;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::system::server::CliConfigView,
    CliError,
};

use super::CONFIG_PATH;

#[derive(Debug, Clone)]
pub struct View {
    pub backend: Backend,
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

#[async_trait]
impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        self.backend.get(CONFIG_PATH).await?;
        Ok(())
    }
}
