use crate::{bencher::sub::SubCmd, parser::organization::member::CliMember, CliError};

mod invite;
mod list;
mod remove;
mod update;
mod view;

#[derive(Debug)]
pub enum Member {
    List(list::List),
    Invite(invite::Invite),
    View(view::View),
    Update(update::Update),
    Remove(remove::Remove),
}

impl TryFrom<CliMember> for Member {
    type Error = CliError;

    fn try_from(member: CliMember) -> Result<Self, Self::Error> {
        Ok(match member {
            CliMember::List(list) => Self::List(list.try_into()?),
            CliMember::Invite(invite) => Self::Invite(invite.try_into()?),
            CliMember::View(view) => Self::View(view.try_into()?),
            CliMember::Update(update) => Self::Update(update.try_into()?),
            CliMember::Remove(remove) => Self::Remove(remove.try_into()?),
        })
    }
}

impl SubCmd for Member {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::Invite(invite) => invite.exec().await,
            Self::View(view) => view.exec().await,
            Self::Update(update) => update.exec().await,
            Self::Remove(remove) => remove.exec().await,
        }
    }
}
