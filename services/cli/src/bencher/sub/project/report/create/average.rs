use bencher_client::types::JsonAverage;

use crate::parser::project::report::CliReportAverage;

impl From<CliReportAverage> for JsonAverage {
    fn from(average: CliReportAverage) -> Self {
        match average {
            CliReportAverage::Mean => Self::Mean,
            CliReportAverage::Median => Self::Median,
        }
    }
}
