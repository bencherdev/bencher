use async_trait::async_trait;

use crate::{
    bencher::{sub::SubCmd, wide::Wide},
    cli::benchmark::CliBenchmark,
    BencherError,
};

mod list;
mod view;

#[derive(Debug)]
pub enum Benchmark {
    List(list::List),
    View(view::View),
}

impl TryFrom<CliBenchmark> for Benchmark {
    type Error = BencherError;

    fn try_from(benchmark: CliBenchmark) -> Result<Self, Self::Error> {
        Ok(match benchmark {
            CliBenchmark::List(list) => Self::List(list.try_into()?),
            CliBenchmark::View(view) => Self::View(view.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Benchmark {
    async fn exec(&self, wide: &Wide) -> Result<(), BencherError> {
        match self {
            Self::List(list) => list.exec(wide).await,
            Self::View(create) => create.exec(wide).await,
        }
    }
}
