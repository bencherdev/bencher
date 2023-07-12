use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonOrganizationPermission;
use bencher_json::{JsonAllowed, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::organization::{CliOrganizationAllowed, CliOrganizationPermission},
    CliError,
};

#[derive(Debug)]
pub struct Allowed {
    pub organization: ResourceId,
    pub perm: Permission,
    pub backend: Backend,
}

impl TryFrom<CliOrganizationAllowed> for Allowed {
    type Error = CliError;

    fn try_from(allowed: CliOrganizationAllowed) -> Result<Self, Self::Error> {
        let CliOrganizationAllowed {
            organization,
            perm,
            backend,
        } = allowed;
        Ok(Self {
            organization,
            perm: perm.into(),
            backend: backend.try_into()?,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Permission {
    View,
    Create,
    Edit,
    Delete,
    Manage,
    ViewRole,
    CreateRole,
    EditRole,
    DeleteRole,
}

impl From<CliOrganizationPermission> for Permission {
    fn from(permission: CliOrganizationPermission) -> Self {
        match permission {
            CliOrganizationPermission::View => Self::View,
            CliOrganizationPermission::Create => Self::Create,
            CliOrganizationPermission::Edit => Self::Edit,
            CliOrganizationPermission::Delete => Self::Delete,
            CliOrganizationPermission::Manage => Self::Manage,
            CliOrganizationPermission::ViewRole => Self::ViewRole,
            CliOrganizationPermission::CreateRole => Self::CreateRole,
            CliOrganizationPermission::EditRole => Self::EditRole,
            CliOrganizationPermission::DeleteRole => Self::DeleteRole,
        }
    }
}

impl From<Permission> for JsonOrganizationPermission {
    fn from(permission: Permission) -> Self {
        match permission {
            Permission::View => Self::View,
            Permission::Create => Self::Create,
            Permission::Edit => Self::Edit,
            Permission::Delete => Self::Delete,
            Permission::Manage => Self::Manage,
            Permission::ViewRole => Self::ViewRole,
            Permission::CreateRole => Self::CreateRole,
            Permission::EditRole => Self::EditRole,
            Permission::DeleteRole => Self::DeleteRole,
        }
    }
}

#[async_trait]
impl SubCmd for Allowed {
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonAllowed = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .org_allowed_get()
                        .organization(self.organization.clone())
                        .permission(self.perm)
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
