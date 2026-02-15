use crate::{
    CliError,
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::system::server::CliOpenApiSpec,
};

#[derive(Debug, Clone)]
pub struct OpenApiSpec {
    pub backend: PubBackend,
}

impl TryFrom<CliOpenApiSpec> for OpenApiSpec {
    type Error = CliError;

    fn try_from(spec: CliOpenApiSpec) -> Result<Self, Self::Error> {
        let CliOpenApiSpec { backend } = spec;
        Ok(Self {
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for OpenApiSpec {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move { client.server_spec_get().send().await })
            .await?;
        Ok(())
    }
}
