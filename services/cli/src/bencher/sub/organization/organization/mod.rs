use crate::{bencher::sub::SubCmd, parser::organization::CliOrganization, CliError};

mod allowed;
mod create;
mod delete;
mod list;
mod update;
mod usage;
mod view;

#[derive(Debug)]
pub enum Organization {
    List(list::List),
    Create(create::Create),
    View(view::View),
    Update(update::Update),
    Delete(delete::Delete),
    Allowed(allowed::Allowed),
    #[cfg(feature = "plus")]
    Usage(usage::Usage),
}

impl TryFrom<CliOrganization> for Organization {
    type Error = CliError;

    fn try_from(branch: CliOrganization) -> Result<Self, Self::Error> {
        Ok(match branch {
            CliOrganization::List(list) => Self::List(list.try_into()?),
            CliOrganization::Create(create) => Self::Create(create.try_into()?),
            CliOrganization::View(view) => Self::View(view.try_into()?),
            CliOrganization::Update(update) => Self::Update(update.try_into()?),
            CliOrganization::Delete(delete) => Self::Delete(delete.try_into()?),
            CliOrganization::Allowed(allowed) => Self::Allowed(allowed.try_into()?),
            #[cfg(feature = "plus")]
            CliOrganization::Usage(usage) => Self::Usage(usage.try_into()?),
        })
    }
}

impl SubCmd for Organization {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::Create(create) => create.exec().await,
            Self::View(view) => view.exec().await,
            Self::Update(update) => update.exec().await,
            Self::Delete(delete) => delete.exec().await,
            Self::Allowed(allowed) => allowed.exec().await,
            #[cfg(feature = "plus")]
            Self::Usage(usage) => usage.exec().await,
        }
    }
}
