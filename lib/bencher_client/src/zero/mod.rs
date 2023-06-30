use crate::{to_endpoint::impl_display, ToEndpoint};

pub mod organizations;

pub use organizations::Organizations;

#[derive(Clone)]
pub enum Zero {
    Organizations(Option<Organizations>),
}

impl_display!(Zero);

impl ToEndpoint for Zero {
    fn to_endpoint(&self) -> String {
        match self {
            Self::Organizations(resource) => Self::resource("organizations", resource),
        }
    }
}
