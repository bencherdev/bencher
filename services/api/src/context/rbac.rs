use bencher_rbac::{Organization, Project};
use oso::{Oso, ToPolar};

use crate::{model::user::auth::AuthUser, ApiError};

pub struct Rbac(pub Oso);

impl From<Oso> for Rbac {
    fn from(oso: Oso) -> Self {
        Self(oso)
    }
}

impl Rbac {
    pub fn is_allowed<Actor, Action, Resource>(
        &self,
        actor: Actor,
        action: Action,
        resource: Resource,
    ) -> Result<bool, ApiError>
    where
        Actor: ToPolar,
        Action: ToPolar,
        Resource: ToPolar,
    {
        self.0
            .is_allowed(actor, action, resource)
            .map_err(ApiError::IsAllowed)
    }

    pub fn is_allowed_unwrap<Actor, Action, Resource>(
        &self,
        actor: Actor,
        action: Action,
        resource: Resource,
    ) -> bool
    where
        Actor: ToPolar,
        Action: ToPolar,
        Resource: ToPolar,
    {
        // If there is an error or if the bool is false, then false
        // Otherwise, if the bool is true, then true
        self.is_allowed(actor, action, resource).unwrap_or_default()
    }

    pub fn is_allowed_organization(
        &self,
        auth_user: &AuthUser,
        permission: bencher_rbac::organization::Permission,
        organization: impl Into<Organization>,
    ) -> Result<(), ApiError> {
        let organization = organization.into();
        self.is_allowed_unwrap(auth_user, permission, organization.clone())
            .then_some(())
            .ok_or_else(|| ApiError::IsAllowedOrganization {
                auth_user: auth_user.clone(),
                permission,
                organization,
            })
    }

    pub fn is_allowed_project(
        &self,
        auth_user: &AuthUser,
        permission: bencher_rbac::project::Permission,
        project: impl Into<Project>,
    ) -> Result<(), ApiError> {
        let project = project.into();
        self.is_allowed_unwrap(auth_user, permission, project.clone())
            .then_some(())
            .ok_or_else(|| ApiError::IsAllowedProject {
                auth_user: auth_user.clone(),
                permission,
                project,
            })
    }
}
