use clap::Parser;

#[derive(Parser, Debug)]
pub struct TaskFlyTest {
    /// Run devel tests
    #[clap(long)]
    pub dev: bool,
}

#[derive(Parser, Debug)]
pub struct TaskNetlifyTest {
    /// Run devel tests
    #[clap(long)]
    pub dev: bool,
}
