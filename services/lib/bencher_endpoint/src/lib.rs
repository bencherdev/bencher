use bencher_json::organization::organization::JsonOrganizationPermission;
use std::fmt;

pub trait ToEndpoint {
    fn to_endpoint(&self) -> String;

    fn resource<Resource>(resource_str: impl AsRef<str>, resource: &Resource) -> String
    where
        Resource: ToEndpoint,
    {
        format!("{}{}", resource_str.as_ref(), resource.to_endpoint())
    }
}

impl<T> ToEndpoint for Option<T>
where
    T: ToEndpoint,
{
    fn to_endpoint(&self) -> String {
        if let Some(t) = self {
            Self::resource("/", t)
        } else {
            String::default()
        }
    }
}

#[derive(Clone)]
pub struct PathParam<Resource>(String, Option<Resource>);

impl<Resource> ToEndpoint for PathParam<Resource>
where
    Resource: ToEndpoint,
{
    fn to_endpoint(&self) -> String {
        Self::resource(&self.0, &self.1)
    }
}

#[derive(Clone, Copy)]
pub struct Resource;

impl ToEndpoint for Resource {
    fn to_endpoint(&self) -> String {
        String::default()
    }
}

#[derive(Clone)]
pub enum Endpoint {
    Zero(Option<Zero>),
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/{}", self.to_endpoint())
    }
}

impl ToEndpoint for Endpoint {
    fn to_endpoint(&self) -> String {
        match self {
            Self::Zero(resource) => Self::resource("v0", resource),
        }
    }
}

#[derive(Clone)]
pub enum Zero {
    Organizations(Option<Organizations>),
}

impl ToEndpoint for Zero {
    fn to_endpoint(&self) -> String {
        match self {
            Self::Organizations(resource) => Self::resource("organizations", resource),
        }
    }
}

pub type Organizations = PathParam<Organization>;

#[derive(Clone)]
pub enum Organization {
    Members(Option<PathParam<Resource>>),
    Allowed(Option<JsonOrganizationPermission>),
    Projects(Option<PathParam<Resource>>),
}

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

#[cfg(test)]
mod test {
    use super::*;
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
