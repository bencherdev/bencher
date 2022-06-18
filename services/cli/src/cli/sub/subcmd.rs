use async_trait::async_trait;

use crate::cli::wide::Wide;
use crate::BencherError;

#[async_trait]
pub trait SubCmd {
    async fn run(&self, wide: &Wide) -> Result<(), BencherError>;
}
