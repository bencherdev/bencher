use std::convert::TryFrom;

use bencher_client::types::ProjectPermission;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::{CliProjectAllowed, CliProjectPermission},
    CliError,
};

#[derive(Debug)]
pub struct Allowed {
    pub project: ResourceId,
    pub perm: ProjectPermission,
    pub backend: AuthBackend,
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

impl From<CliProjectPermission> for ProjectPermission {
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

impl SubCmd for Allowed {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_allowed_get()
                    .project(self.project.clone())
                    .permission(self.perm)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
