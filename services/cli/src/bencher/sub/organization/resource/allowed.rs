use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{organization::JsonOrganizationPermission, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::organization::{CliOrganizationAllowed, CliOrganizationPermission},
    CliError,
};

#[derive(Debug)]
pub struct Allowed {
    pub perm: JsonOrganizationPermission,
    pub organization: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliOrganizationAllowed> for Allowed {
    type Error = CliError;

    fn try_from(allowed: CliOrganizationAllowed) -> Result<Self, Self::Error> {
        let CliOrganizationAllowed {
            perm,
            organization,
            backend,
        } = allowed;
        Ok(Self {
            perm: perm.into(),
            organization,
            backend: backend.try_into()?,
        })
    }
}

impl From<CliOrganizationPermission> for JsonOrganizationPermission {
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
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        self.backend
            .get(&format!(
                "/v0/organizations/{}/allowed/{}",
                self.organization, self.perm
            ))
            .await?;
        Ok(())
    }
}
