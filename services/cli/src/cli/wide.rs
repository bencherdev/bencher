use crate::cli::clap::CliWide;

#[derive(Debug)]
pub struct Wide {}

impl From<CliWide> for Wide {
    fn from(_wide: CliWide) -> Self {
        Wide {}
    }
}
