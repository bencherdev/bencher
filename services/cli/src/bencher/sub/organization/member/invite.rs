use bencher_client::types::{JsonNewMember, OrganizationRole};
use bencher_json::{Email, OrganizationResourceId, UserName};

use crate::{
    CliError,
    bencher::backend::AuthBackend,
    parser::organization::member::{CliMemberInvite, CliMemberRole},
};

use crate::bencher::SubCmd;

#[derive(Debug, Clone)]
pub struct Invite {
    organization: OrganizationResourceId,
    name: Option<UserName>,
    email: Email,
    role: OrganizationRole,
    backend: AuthBackend,
}

impl TryFrom<CliMemberInvite> for Invite {
    type Error = CliError;

    fn try_from(invite: CliMemberInvite) -> Result<Self, Self::Error> {
        let CliMemberInvite {
            organization,
            name,
            email,
            role,
            backend,
        } = invite;
        Ok(Self {
            organization,
            name,
            email,
            role: role.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliMemberRole> for OrganizationRole {
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
        Self {
            name: name.map(Into::into),
            email: email.into(),
            role,
        }
    }
}

impl SubCmd for Invite {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .org_member_post()
                    .organization(self.organization.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
