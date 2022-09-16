use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{invite::JsonInviteRole, JsonInvite};
use uuid::Uuid;

use crate::{
    bencher::{backend::Backend, wide::Wide},
    cli::invite::{CliInvite, CliInviteRole},
    CliError,
};

use super::SubCmd;

const INVITES_PATH: &str = "/v0/invites";

#[derive(Debug, Clone)]
pub struct Invite {
    email: String,
    org: Uuid,
    role: JsonInviteRole,
    backend: Backend,
}

impl TryFrom<CliInvite> for Invite {
    type Error = CliError;

    fn try_from(invite: CliInvite) -> Result<Self, Self::Error> {
        let CliInvite {
            email,
            org,
            role,
            backend,
        } = invite;
        Ok(Self {
            email,
            org,
            role: role.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliInviteRole> for JsonInviteRole {
    fn from(role: CliInviteRole) -> Self {
        match role {
            CliInviteRole::Member => Self::Member,
            CliInviteRole::Leader => Self::Leader,
        }
    }
}

impl From<Invite> for JsonInvite {
    fn from(invite: Invite) -> Self {
        let Invite {
            email,
            org,
            role,
            backend: _,
        } = invite;
        Self {
            email,
            organization: org,
            role,
        }
    }
}

#[async_trait]
impl SubCmd for Invite {
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        let invite: JsonInvite = self.clone().into();
        self.backend.post(INVITES_PATH, &invite).await?;
        Ok(())
    }
}
