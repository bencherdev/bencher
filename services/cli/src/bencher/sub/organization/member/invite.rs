use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{
    organization::member::{JsonNewMember, JsonOrganizationRole},
    Email, ResourceId, UserName,
};

use crate::{
    bencher::backend::Backend,
    cli::organization::member::{CliMemberInvite, CliMemberRole},
    CliError,
};

use crate::bencher::SubCmd;

#[derive(Debug, Clone)]
pub struct Invite {
    org: ResourceId,
    name: Option<UserName>,
    email: Email,
    role: JsonOrganizationRole,
    backend: Backend,
}

impl TryFrom<CliMemberInvite> for Invite {
    type Error = CliError;

    fn try_from(invite: CliMemberInvite) -> Result<Self, Self::Error> {
        let CliMemberInvite {
            org,
            name,
            email,
            role,
            backend,
        } = invite;
        Ok(Self {
            org,
            name,
            email,
            role: role.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliMemberRole> for JsonOrganizationRole {
    fn from(role: CliMemberRole) -> Self {
        match role {
            // TODO Team Management
            // CliMemberRole::Member => Self::Member,
            CliMemberRole::Leader => Self::Leader,
        }
    }
}

impl From<Invite> for JsonNewMember {
    fn from(invite: Invite) -> Self {
        let Invite {
            name, email, role, ..
        } = invite;
        Self { name, email, role }
    }
}

#[async_trait]
impl SubCmd for Invite {
    async fn exec(&self) -> Result<(), CliError> {
        let invite: JsonNewMember = self.clone().into();
        self.backend
            .post(&format!("/v0/organizations/{}/members", &self.org), &invite)
            .await?;
        Ok(())
    }
}
