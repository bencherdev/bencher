use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::OrganizationPermission;
use bencher_json::{JsonAllowed, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::organization::{CliOrganizationAllowed, CliOrganizationPermission},
    CliError,
};

#[derive(Debug)]
pub struct Allowed {
    pub organization: ResourceId,
    pub perm: OrganizationPermission,
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

#[async_trait]
impl SubCmd for Allowed {
    async fn exec(&self) -> Result<(), CliError> {
        let _json: JsonAllowed = self
            .backend
            .send_with(|client| async move {
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
