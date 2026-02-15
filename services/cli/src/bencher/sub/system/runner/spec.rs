use bencher_client::types::JsonNewRunnerSpec;
use bencher_json::{RunnerResourceId, SpecResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::runner::{
        CliRunnerSpec, CliRunnerSpecAdd, CliRunnerSpecList, CliRunnerSpecRemove,
    },
};

#[derive(Debug)]
pub enum Spec {
    List(List),
    Add(Add),
    Remove(Remove),
}

impl TryFrom<CliRunnerSpec> for Spec {
    type Error = CliError;

    fn try_from(spec: CliRunnerSpec) -> Result<Self, Self::Error> {
        Ok(match spec {
            CliRunnerSpec::List(list) => Self::List(list.try_into()?),
            CliRunnerSpec::Add(add) => Self::Add(add.try_into()?),
            CliRunnerSpec::Remove(remove) => Self::Remove(remove.try_into()?),
        })
    }
}

impl SubCmd for Spec {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::Add(add) => add.exec().await,
            Self::Remove(remove) => remove.exec().await,
        }
    }
}

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
