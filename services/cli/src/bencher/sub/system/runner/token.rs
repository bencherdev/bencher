use bencher_json::RunnerResourceId;

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::runner::CliRunnerToken,
};

#[derive(Debug)]
pub struct Token {
    pub runner: RunnerResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliRunnerToken> for Token {
    type Error = CliError;

    fn try_from(token: CliRunnerToken) -> Result<Self, Self::Error> {
        let CliRunnerToken { runner, backend } = token;
        Ok(Self {
            runner,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Token {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .runner_token_post()
                    .runner(self.runner.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
