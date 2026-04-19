use bencher_json::RunnerResourceId;

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::runner::CliRunnerKey,
};

#[derive(Debug)]
pub struct Key {
    pub runner: RunnerResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliRunnerKey> for Key {
    type Error = CliError;

    fn try_from(key: CliRunnerKey) -> Result<Self, Self::Error> {
        let CliRunnerKey { runner, backend } = key;
        Ok(Self {
            runner,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Key {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .runner_key_post()
                    .runner(self.runner.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
