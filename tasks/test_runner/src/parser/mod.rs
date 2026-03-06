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
}
