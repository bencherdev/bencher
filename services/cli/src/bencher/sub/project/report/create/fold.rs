use bencher_client::types::JsonFold;

use crate::parser::project::report::CliReportFold;

impl From<CliReportFold> for JsonFold {
    fn from(fold: CliReportFold) -> Self {
        match fold {
            CliReportFold::Min => Self::Min,
            CliReportFold::Max => Self::Max,
            CliReportFold::Mean => Self::Mean,
            CliReportFold::Median => Self::Median,
        }
    }
}
