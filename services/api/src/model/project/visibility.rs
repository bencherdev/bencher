use bencher_json::project::JsonVisibility;

use crate::ApiError;

const PUBLIC_INT: i32 = 0;
#[cfg(feature = "plus")]
const PRIVATE_INT: i32 = 1;

#[repr(i32)]
pub enum Visibility {
    Public = PUBLIC_INT,
    #[cfg(feature = "plus")]
    Private = PRIVATE_INT,
}

impl TryFrom<i32> for Visibility {
    type Error = ApiError;

    fn try_from(visibility: i32) -> Result<Self, Self::Error> {
        match visibility {
            PUBLIC_INT => Ok(Self::Public),
            #[cfg(feature = "plus")]
            PRIVATE_INT => Ok(Self::Private),
            _ => Err(ApiError::VisibilityInt(visibility)),
        }
    }
}

impl From<JsonVisibility> for Visibility {
    fn from(visibility: JsonVisibility) -> Self {
        match visibility {
            JsonVisibility::Public => Self::Public,
            #[cfg(feature = "plus")]
            JsonVisibility::Private => Self::Private,
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

impl Visibility {
    pub fn is_public(self) -> bool {
        JsonVisibility::from(self).is_public()
    }
}
