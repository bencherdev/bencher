use bencher_json::organization::organization::JsonOrganizationPermission;

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

pub enum OrganizationResource {
    Members(Option<Members>),
    Allowed(Option<JsonOrganizationPermission>),
    Projects(Option<Projects>),
}

pub type Members = PathParam<Member, MemberResource>;

pub struct Member;

pub enum MemberResource {}

pub type Projects = PathParam<Project, ProjectResource>;

pub struct Project;

pub enum ProjectResource {}
