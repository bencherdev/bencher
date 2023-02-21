use bencher_json::PlanLevel;

use crate::cli::organization::plan::CliPlanLevel;

#[derive(Debug, Clone)]
pub enum Level {
    Free,
    Team,
    Enterprise,
}

impl From<CliPlanLevel> for Level {
    fn from(level: CliPlanLevel) -> Self {
        match level {
            CliPlanLevel::Free => Self::Free,
            CliPlanLevel::Team => Self::Team,
            CliPlanLevel::Enterprise => Self::Enterprise,
        }
    }
}

impl From<Level> for PlanLevel {
    fn from(level: Level) -> Self {
        match level {
            Level::Free => Self::Free,
            Level::Team => Self::Team,
            Level::Enterprise => Self::Enterprise,
        }
    }
}
