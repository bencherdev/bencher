use async_trait::async_trait;

use crate::{
    bencher::{sub::SubCmd, wide::Wide},
    cli::member::CliMember,
    CliError,
};

mod list;
mod view;

#[derive(Debug)]
pub enum Member {
    List(list::List),
    View(view::View),
}

impl TryFrom<CliMember> for Member {
    type Error = CliError;

    fn try_from(member: CliMember) -> Result<Self, Self::Error> {
        Ok(match member {
            CliMember::List(list) => Self::List(list.try_into()?),
            CliMember::View(view) => Self::View(view.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Member {
    async fn exec(&self, wide: &Wide) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec(wide).await,
            Self::View(view) => view.exec(wide).await,
        }
    }
}
