use bencher_json::invite::JsonInviteRole;

use crate::cli::invite::CliInviteRole;

#[derive(Clone, Copy, Debug)]
pub enum Role {
    Member,
    Leader,
}

impl From<CliInviteRole> for Role {
    fn from(role: CliInviteRole) -> Self {
        match role {
            CliInviteRole::Member => Self::Member,
            CliInviteRole::Leader => Self::Leader,
        }
    }
}

impl From<Role> for JsonInviteRole {
    fn from(kind: Role) -> Self {
        match kind {
            Role::Member => Self::Member,
            Role::Leader => Self::Leader,
        }
    }
}
