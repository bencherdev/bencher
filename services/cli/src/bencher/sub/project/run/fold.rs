use bencher_client::types::JsonFold;

use crate::parser::project::run::CliRunFold;

#[derive(Debug, Clone, Copy)]
pub enum Fold {
    Min,
    Max,
    Mean,
    Median,
}

impl From<CliRunFold> for Fold {
    fn from(fold: CliRunFold) -> Self {
        match fold {
            CliRunFold::Min => Self::Min,
            CliRunFold::Max => Self::Max,
            CliRunFold::Mean => Self::Mean,
            CliRunFold::Median => Self::Median,
        }
    }
}

impl From<Fold> for JsonFold {
    fn from(fold: Fold) -> Self {
        match fold {
            Fold::Min => Self::Min,
            Fold::Max => Self::Max,
            Fold::Mean => Self::Mean,
            Fold::Median => Self::Median,
        }
    }
}
