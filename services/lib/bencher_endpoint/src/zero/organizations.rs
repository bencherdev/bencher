use bencher_json::organization::organization::JsonOrganizationPermission;

use crate::{to_endpoint::impl_display, PathParam, Resource, ToEndpoint};

pub type Organizations = PathParam<Organization>;

#[derive(Clone)]
pub enum Organization {
    Members(Option<PathParam<Resource>>),
    Allowed(Option<JsonOrganizationPermission>),
    Projects(Option<PathParam<Resource>>),
}

impl_display!(Organization);

impl ToEndpoint for Organization {
    fn to_endpoint(&self) -> String {
        match self {
            Self::Members(resource) => Self::resource("members", resource),
            Self::Allowed(resource) => Self::resource("allowed", resource),
            Self::Projects(resource) => Self::resource("projects", resource),
        }
    }
}

impl ToEndpoint for JsonOrganizationPermission {
    fn to_endpoint(&self) -> String {
        self.to_string()
    }
}
