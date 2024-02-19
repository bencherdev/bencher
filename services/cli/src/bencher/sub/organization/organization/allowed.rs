use bencher_client::types::OrganizationPermission;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::{CliOrganizationAllowed, CliOrganizationPermission},
    CliError,
};

#[derive(Debug)]
pub struct Allowed {
    pub organization: ResourceId,
    pub perm: OrganizationPermission,
    pub backend: AuthBackend,
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

impl From<CliOrganizationPermission> for OrganizationPermission {
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

impl SubCmd for Allowed {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .org_allowed_get()
                    .organization(self.organization.clone())
                    .permission(self.perm)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
