use async_trait::async_trait;

use crate::{bencher::wide::Wide, CliError};

#[async_trait]
pub trait SubCmd {
    async fn exec(&self, wide: &Wide) -> Result<(), CliError>;
}
