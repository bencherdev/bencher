use clap::Parser;

#[derive(Parser, Debug)]
pub struct CliFlyTest {
    /// Run devel tests
    #[clap(long)]
    pub dev: bool,
}

#[derive(Parser, Debug)]
pub struct CliNetlifyTest {
    /// Run devel tests
    #[clap(long)]
    pub dev: bool,
}
