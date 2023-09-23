use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonProjectPermission;
use bencher_json::{JsonAllowed, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::{CliProjectAllowed, CliProjectPermission},
    CliError,
};

#[derive(Debug)]
pub struct Allowed {
    pub project: ResourceId,
    pub perm: Permission,
    pub backend: Backend,
}

impl TryFrom<CliProjectAllowed> for Allowed {
    type Error = CliError;

    fn try_from(allowed: CliProjectAllowed) -> Result<Self, Self::Error> {
        let CliProjectAllowed {
            project,
            perm,
            backend,
        } = allowed;
        Ok(Self {
            project,
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

impl From<CliProjectPermission> for Permission {
    fn from(permission: CliProjectPermission) -> Self {
        match permission {
            CliProjectPermission::View => Self::View,
            CliProjectPermission::Create => Self::Create,
            CliProjectPermission::Edit => Self::Edit,
            CliProjectPermission::Delete => Self::Delete,
            CliProjectPermission::Manage => Self::Manage,
            CliProjectPermission::ViewRole => Self::ViewRole,
            CliProjectPermission::CreateRole => Self::CreateRole,
            CliProjectPermission::EditRole => Self::EditRole,
            CliProjectPermission::DeleteRole => Self::DeleteRole,
        }
    }
}

impl From<Permission> for JsonProjectPermission {
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
        let _json: JsonAllowed = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_allowed_get()
                        .project(self.project.clone())
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
