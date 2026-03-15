use clap::{Parser, Subcommand};

mod live;
mod plus;

pub use live::TaskLive;
#[cfg(feature = "plus")]
pub use plus::{
    email_list::TaskEmailList,
    index::{TaskIndex, TaskIndexDelete, TaskIndexUpdate, TaskSearchEngine},
    license::{TaskBillingCycle, TaskLicense, TaskLicenseGenerate, TaskLicenseValidate},
    stats::TaskStats,
};

#[derive(Parser, Debug)]
pub struct TaskTask {
    #[clap(subcommand)]
    pub sub: TaskSub,
}

#[derive(Subcommand, Debug)]
pub enum TaskSub {
    /// Live API version
    Live(TaskLive),
    #[cfg(feature = "plus")]
    #[clap(subcommand)]
    /// `URLindexing`
    Index(TaskIndex),
    #[cfg(feature = "plus")]
    /// Send stats to bencher.dev
    Stats(TaskStats),
    #[cfg(feature = "plus")]
    #[clap(subcommand)]
    /// License management
    License(TaskLicense),
    #[cfg(feature = "plus")]
    /// Generate email list
    EmailList(TaskEmailList),
}
