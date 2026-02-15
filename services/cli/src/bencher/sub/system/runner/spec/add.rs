use bencher_client::types::JsonNewRunnerSpec;
use bencher_json::{RunnerResourceId, SpecResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::runner::CliRunnerSpecAdd,
};

#[derive(Debug)]
pub struct Add {
    pub runner: RunnerResourceId,
    pub spec: SpecResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliRunnerSpecAdd> for Add {
    type Error = CliError;

    fn try_from(add: CliRunnerSpecAdd) -> Result<Self, Self::Error> {
        let CliRunnerSpecAdd {
            runner,
            spec,
            backend,
        } = add;
        Ok(Self {
            runner,
            spec,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Add {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .runner_specs_post()
                    .runner(self.runner.clone())
                    .body(JsonNewRunnerSpec {
                        spec: self.spec.clone().into(),
                    })
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
