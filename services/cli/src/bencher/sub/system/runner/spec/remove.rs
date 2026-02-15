use bencher_json::{RunnerResourceId, SpecResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::runner::CliRunnerSpecRemove,
};

#[derive(Debug)]
pub struct Remove {
    pub runner: RunnerResourceId,
    pub spec: SpecResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliRunnerSpecRemove> for Remove {
    type Error = CliError;

    fn try_from(remove: CliRunnerSpecRemove) -> Result<Self, Self::Error> {
        let CliRunnerSpecRemove {
            runner,
            spec,
            backend,
        } = remove;
        Ok(Self {
            runner,
            spec,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Remove {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .runner_spec_delete()
                    .runner(self.runner.clone())
                    .spec(self.spec.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
