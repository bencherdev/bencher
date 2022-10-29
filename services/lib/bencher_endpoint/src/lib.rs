use bencher_json::organization::organization::JsonOrganizationPermission;
use std::fmt;

#[derive(Clone, Copy)]
pub struct Endpoint(pub Option<Version>);

#[derive(Clone, Copy)]
pub struct PathParam<Param, Resource>(Param, Option<Resource>);

#[derive(Clone, Copy)]
pub enum Version {
    Zero(Option<Zero>),
}

#[derive(Clone, Copy)]
pub enum Zero {
    Organizations(Option<Organizations>),
}

pub type Organizations = PathParam<Organization, OrganizationResource>;

#[derive(Clone, Copy)]
pub struct Organization;

#[derive(Clone, Copy)]
pub enum OrganizationResource {
    Members(Option<Members>),
    Allowed(Option<JsonOrganizationPermission>),
    Projects(Option<Projects>),
}

pub type Members = PathParam<Member, MemberResource>;

#[derive(Clone, Copy)]
pub struct Member;

#[derive(Clone, Copy)]
pub enum MemberResource {}

pub type Projects = PathParam<Project, ProjectResource>;

#[derive(Clone, Copy)]
pub struct Project;

#[derive(Clone, Copy)]
pub enum ProjectResource {}
