use crate::CliError;

pub trait SubCmd {
    async fn exec(&self) -> Result<(), CliError>;
}
