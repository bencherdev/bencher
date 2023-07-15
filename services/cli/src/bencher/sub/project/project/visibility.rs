use bencher_client::types::JsonVisibility;

use crate::parser::project::CliProjectVisibility;

#[derive(Debug, Clone, Copy)]
pub enum Visibility {
    Public,
    #[cfg(feature = "plus")]
    Private,
}

impl From<CliProjectVisibility> for Visibility {
    fn from(visibility: CliProjectVisibility) -> Self {
        match visibility {
            CliProjectVisibility::Public => Self::Public,
            #[cfg(feature = "plus")]
            CliProjectVisibility::Private => Self::Private,
        }
    }
}

impl From<Visibility> for JsonVisibility {
    fn from(visibility: Visibility) -> Self {
        match visibility {
            Visibility::Public => Self::Public,
            #[cfg(feature = "plus")]
            Visibility::Private => Self::Private,
        }
    }
}
