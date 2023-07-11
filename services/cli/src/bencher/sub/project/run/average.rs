use bencher_client::types::JsonAverage;

use crate::cli::project::run::CliRunAverage;

#[derive(Debug, Clone, Copy)]
pub enum Average {
    Mean,
    Median,
}

impl From<CliRunAverage> for Average {
    fn from(average: CliRunAverage) -> Self {
        match average {
            CliRunAverage::Mean => Self::Mean,
            CliRunAverage::Median => Self::Median,
        }
    }
}

impl From<Average> for JsonAverage {
    fn from(average: Average) -> Self {
        match average {
            Average::Mean => Self::Mean,
            Average::Median => Self::Median,
        }
    }
}
