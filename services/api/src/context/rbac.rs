use bencher_rbac::{Organization, Project};
use oso::{Oso, ToPolar};

use crate::model::user::auth::AuthUser;

pub struct Rbac(pub Oso);

impl From<Oso> for Rbac {
    fn from(oso: Oso) -> Self {
        Self(oso)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RbacError {
    #[error("Failed to check permissions: {0}")]
    IsAllowed(oso::OsoError),
    #[error("Permission ({permission}) denied for user ({auth_user:?}) on organization ({organization:?})")]
    IsAllowedOrganization {
        auth_user: AuthUser,
        permission: bencher_rbac::organization::Permission,
        organization: Organization,
    },
    #[error("Permission ({permission}) denied for user ({auth_user:?}) on project ({project:?})")]
    IsAllowedProject {
        auth_user: AuthUser,
        permission: bencher_rbac::project::Permission,
        project: Project,
    },
}

impl Rbac {
    pub fn is_allowed<Actor, Action, Resource>(
        &self,
        actor: Actor,
        action: Action,
        resource: Resource,
    ) -> Result<bool, RbacError>
    where
        Actor: ToPolar,
        Action: ToPolar,
        Resource: ToPolar,
    {
        self.0.is_allowed(actor, action, resource).map_err(|e| {
            #[cfg(feature = "sentry")]
            sentry::capture_error(&e);
            RbacError::IsAllowed(e)
        })
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

    pub fn is_allowed_organization<O>(
        &self,
        auth_user: &AuthUser,
        permission: bencher_rbac::organization::Permission,
        organization: O,
    ) -> Result<(), RbacError>
    where
        O: Into<Organization>,
    {
        let organization = organization.into();
        self.is_allowed_unwrap(auth_user, permission, organization.clone())
            .then_some(())
            .ok_or_else(|| RbacError::IsAllowedOrganization {
                auth_user: auth_user.clone(),
                permission,
                organization,
            })
    }

    pub fn is_allowed_project<P>(
        &self,
        auth_user: &AuthUser,
        permission: bencher_rbac::project::Permission,
        project: P,
    ) -> Result<(), RbacError>
    where
        P: Into<Project>,
    {
        let project = project.into();
        self.is_allowed_unwrap(auth_user, permission, project.clone())
            .then_some(())
            .ok_or_else(|| RbacError::IsAllowedProject {
                auth_user: auth_user.clone(),
                permission,
                project,
            })
    }
}
