use bencher_client::types::JsonAverage;

use crate::parser::project::run::CliRunAverage;

impl From<CliRunAverage> for JsonAverage {
    fn from(average: CliRunAverage) -> Self {
        match average {
            CliRunAverage::Mean => Self::Mean,
            CliRunAverage::Median => Self::Median,
        }
    }
}
