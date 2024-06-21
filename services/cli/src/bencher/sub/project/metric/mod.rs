use crate::{bencher::sub::SubCmd, parser::project::metric::CliMetric, CliError};

mod view;

#[derive(Debug)]
pub enum Metric {
    View(view::View),
}

impl TryFrom<CliMetric> for Metric {
    type Error = CliError;

    fn try_from(metric: CliMetric) -> Result<Self, Self::Error> {
        Ok(match metric {
            CliMetric::View(view) => Self::View(view.try_into()?),
        })
    }
}

impl SubCmd for Metric {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::View(view) => view.exec().await,
        }
    }
}
