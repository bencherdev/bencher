use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct TaskTask {
    #[clap(subcommand)]
    pub sub: TaskSub,
}

#[derive(Subcommand, Debug)]
pub enum TaskSub {
    /// Run integration test scenarios (requires Linux + KVM + Docker)
    Scenarios(TaskScenarios),
}

#[derive(Parser, Debug)]
pub struct TaskScenarios {
    /// Run a specific scenario by name
    #[clap(long, short)]
    pub scenario: Option<String>,

    /// List all available scenarios
    #[clap(long, short)]
    pub list: bool,

    /// Build the binaries the scenarios need, then exit.
    ///
    /// The scenarios themselves need root, and building as root would leave
    /// cargo's cache and target directory root-owned. This splits the build
    /// out so it can run unprivileged before the elevated run.
    #[clap(long, conflicts_with_all = ["scenario", "list"])]
    pub build_only: bool,
}
