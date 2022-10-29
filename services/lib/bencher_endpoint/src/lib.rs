pub struct Endpoint(pub Option<Version>);

pub struct PathParam<Param, Path>(Param, Option<Path>);

pub enum Version {
    Zero(Option<Zero>),
}

pub enum Zero {
    Organizations(Option<Organizations>),
}

pub type Organizations = PathParam<Organization, OrganizationResource>;

pub struct Organization;

pub enum OrganizationResource {}
