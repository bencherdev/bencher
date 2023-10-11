use bencher_client::types::JsonFold;

use crate::parser::project::run::CliRunFold;

impl From<CliRunFold> for JsonFold {
    fn from(fold: CliRunFold) -> Self {
        match fold {
            CliRunFold::Min => Self::Min,
            CliRunFold::Max => Self::Max,
            CliRunFold::Mean => Self::Mean,
            CliRunFold::Median => Self::Median,
        }
    }
}
