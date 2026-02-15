use bencher_json::RunnerResourceId;

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::runner::CliRunnerSpecList,
};

#[derive(Debug)]
pub struct List {
    pub runner: RunnerResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliRunnerSpecList> for List {
    type Error = CliError;

    fn try_from(list: CliRunnerSpecList) -> Result<Self, Self::Error> {
        let CliRunnerSpecList { runner, backend } = list;
        Ok(Self {
            runner,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for List {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .runner_specs_get()
                    .runner(self.runner.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
