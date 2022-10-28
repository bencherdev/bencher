use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::JsonEmpty;

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
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
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        self.backend.post(CONFIG_PATH, &JsonEmpty {}).await?;
        Ok(())
    }
}
