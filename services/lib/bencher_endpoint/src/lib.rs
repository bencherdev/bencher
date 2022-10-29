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
pub enum Endpoint {
    Zero(Option<Zero>),
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "/{}",
            match self {
                Self::Zero(resource) => {
                    format!("v0{}", resource.to_endpoint())
                },
            }
        )
    }
}

#[derive(Clone)]
pub enum Zero {
    Organizations(Option<Organizations>),
}

impl ToEndpoint for Zero {
    fn to_endpoint(&self) -> String {
        match self {
            Self::Organizations(resource) => {
                format!("organizations{}", resource.to_endpoint())
            },
        }
    }
}

pub type Organizations = PathParam<Organization>;

#[derive(Clone)]
pub enum Organization {
    Members(Option<Members>),
    Allowed(Option<JsonOrganizationPermission>),
    Projects(Option<Projects>),
}

impl ToEndpoint for Organization {
    fn to_endpoint(&self) -> String {
        match self {
            Self::Members(resource) => {
                format!("members{}", resource.to_endpoint())
            },
            Self::Allowed(resource) => {
                format!("allowed{}", resource.to_endpoint())
            },
            Self::Projects(resource) => {
                format!("projects{}", resource.to_endpoint())
            },
        }
    }
}

pub type Members = PathParam<Member>;

#[derive(Clone)]
pub enum Member {}

impl ToEndpoint for Member {
    fn to_endpoint(&self) -> String {
        match self {
            _ => String::default(),
        }
    }
}

pub type Projects = PathParam<Project>;

#[derive(Clone)]
pub enum Project {}

impl ToEndpoint for Project {
    fn to_endpoint(&self) -> String {
        match self {
            _ => String::default(),
        }
    }
}

impl ToEndpoint for JsonOrganizationPermission {
    fn to_endpoint(&self) -> String {
        self.to_string()
    }
}
