use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{
    auth::JsonConfirm,
    JsonToken,
};

use crate::{
    bencher::{
        backend::Backend,
        sub::SubCmd,
        wide::Wide,
    },
    cli::auth::CliAuthConfirm,
    BencherError,
};

const CONFIRM_PATH: &str = "/v0/auth/confirm";

#[derive(Debug, Clone)]
pub struct Confirm {
    pub token:   String,
    pub backend: Backend,
}

impl TryFrom<CliAuthConfirm> for Confirm {
    type Error = BencherError;

    fn try_from(confirm: CliAuthConfirm) -> Result<Self, Self::Error> {
        let CliAuthConfirm { token, host } = confirm;
        let backend = Backend::new(None, host)?;
        Ok(Self { token, backend })
    }
}

#[async_trait]
impl SubCmd for Confirm {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        let json_token: JsonToken = self.token.clone().into();
        let res = self.backend.post(CONFIRM_PATH, &json_token).await?;
        let _: JsonConfirm = serde_json::from_value(res)?;
        Ok(())
    }
}
