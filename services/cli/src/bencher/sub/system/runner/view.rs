use bencher_json::RunnerResourceId;

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::runner::CliRunnerView,
};

#[derive(Debug)]
pub struct View {
    pub runner: RunnerResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliRunnerView> for View {
    type Error = CliError;

    fn try_from(view: CliRunnerView) -> Result<Self, Self::Error> {
        let CliRunnerView { runner, backend } = view;
        Ok(Self {
            runner,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json =
            self.backend
                .send(|client| async move {
                    client.runner_get().runner(self.runner.clone()).send().await
                })
                .await?;
        Ok(())
    }
}
