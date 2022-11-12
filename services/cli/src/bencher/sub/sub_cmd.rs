use async_trait::async_trait;

use crate::CliError;

#[async_trait]
pub trait SubCmd {
    async fn exec(&self) -> Result<(), CliError>;
}
