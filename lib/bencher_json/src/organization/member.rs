use std::{fmt, str::FromStr};

use bencher_valid::{DateTime, Email, Slug, UserName};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::UserUuid;

pub const MEMBER_ROLE: &str = "member";
pub const LEADER_ROLE: &str = "leader";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewMember {
    pub name: Option<UserName>,
    pub email: Email,
    pub role: OrganizationRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMembers(pub Vec<JsonMember>);

crate::from_vec!(JsonMembers[JsonMember]);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMember {
    pub uuid: UserUuid,
    pub name: UserName,
    pub slug: Slug,
    pub email: Email,
    pub role: OrganizationRole,
    pub created: DateTime,
    pub modified: DateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateMember {
    pub role: Option<OrganizationRole>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
#[serde(rename_all = "snake_case")]
pub enum OrganizationRole {
    // TODO Team Management
    // Member,
    Leader,
}

impl FromStr for OrganizationRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            // TODO Team Management
            // MEMBER_ROLE => Ok(Self::Member),
            LEADER_ROLE => Ok(Self::Leader),
            _ => Err(s.into()),
        }
    }
}

impl fmt::Display for OrganizationRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                // TODO Team Management
                // Self::Member => MEMBER_ROLE,
                Self::Leader => LEADER_ROLE,
            }
        )
    }
}

#[cfg(feature = "db")]
mod organization_role {
    use super::{OrganizationRole, LEADER_ROLE};

    #[derive(Debug, thiserror::Error)]
    pub enum OrganizationRoleError {
        #[error("Invalid organization role value: {0}")]
        Invalid(String),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Text, DB> for OrganizationRole
    where
        DB: diesel::backend::Backend,
        for<'a> String: diesel::serialize::ToSql<diesel::sql_types::Text, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Leader => out.set_value(LEADER_ROLE.to_owned()),
            }
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Text, DB> for OrganizationRole
    where
        DB: diesel::backend::Backend,
        String: diesel::deserialize::FromSql<diesel::sql_types::Text, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            let role = String::from_sql(bytes)?;
            match role.as_str() {
                LEADER_ROLE => Ok(Self::Leader),
                _ => Err(Box::new(OrganizationRoleError::Invalid(role))),
            }
        }
    }
}
