mod path_param;
mod resource;
mod to_endpoint;
pub mod zero;

pub use path_param::PathParam;
pub use resource::Resource;
pub(crate) use to_endpoint::impl_display;
pub use to_endpoint::ToEndpoint;
pub use zero::Zero;

#[derive(Clone)]
pub enum Endpoint {
    Zero(Option<Zero>),
}

impl_display!(Endpoint);

impl ToEndpoint for Endpoint {
    fn to_endpoint(&self) -> String {
        match self {
            Self::Zero(resource) => Self::resource("/v0", resource),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{zero::organizations::Organization, Endpoint, PathParam, Resource, Zero};

    use pretty_assertions::assert_eq;

    #[test]
    fn test_endpoint() {
        assert_eq!("/v0", Endpoint::Zero(None).to_string());
        assert_eq!(
            "/v0/organizations",
            Endpoint::Zero(Some(Zero::Organizations(None))).to_string()
        );
        assert_eq!(
            "/v0/organizations/muriel-bagge",
            Endpoint::Zero(Some(Zero::Organizations(Some(PathParam(
                "muriel-bagge".into(),
                None
            )))))
            .to_string()
        );
        assert_eq!(
            "/v0/organizations/muriel-bagge/projects",
            Endpoint::Zero(Some(Zero::Organizations(Some(PathParam(
                "muriel-bagge".into(),
                Some(Organization::Projects(None))
            )))))
            .to_string()
        );
        assert_eq!(
            "/v0/organizations/muriel-bagge/projects/the-computer",
            Endpoint::Zero(Some(Zero::Organizations(Some(PathParam(
                "muriel-bagge".into(),
                Some(Organization::Projects(Some(PathParam(
                    "the-computer".into(),
                    None
                ))))
            )))))
            .to_string()
        );
        assert_eq!(
            "/v0/organizations/muriel-bagge/projects/the-computer/",
            Endpoint::Zero(Some(Zero::Organizations(Some(PathParam(
                "muriel-bagge".into(),
                Some(Organization::Projects(Some(PathParam(
                    "the-computer".into(),
                    Some(Resource)
                ))))
            )))))
            .to_string()
        );
    }
}
