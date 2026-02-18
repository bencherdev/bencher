use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct TaskTask {
    #[clap(subcommand)]
    pub sub: Option<TaskSub>,
}

#[derive(Subcommand, Debug)]
pub enum TaskSub {
    /// Run the full test (default)
    Test(TaskTest),
    /// Run integration test scenarios (requires Linux + KVM + Docker)
    Scenarios(TaskScenarios),
    /// Only create the OCI image
    Oci(TaskOci),
    /// Clean up test artifacts
    Clean(TaskClean),
}

#[derive(Parser, Debug)]
pub struct TaskTest {}

#[derive(Parser, Debug)]
pub struct TaskScenarios {
    /// Run a specific scenario by name
    #[clap(long, short)]
    pub scenario: Option<String>,

    /// List all available scenarios
    #[clap(long, short)]
    pub list: bool,
}

#[derive(Parser, Debug)]
pub struct TaskOci {}

#[derive(Parser, Debug)]
pub struct TaskClean {}
