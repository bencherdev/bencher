use std::{cmp::Ordering, collections::BTreeSet, fmt, str::FromStr};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ValidError;

const PROJECTS_READ: &str = "projects:read";
const PROJECTS_WRITE: &str = "projects:write";
const OPENID: &str = "openid";
const EMAIL: &str = "email";
const PROFILE: &str = "profile";

/// An OAuth scope: what a client may do on the user's behalf.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum OAuthScope {
    /// Read the projects the user can read.
    #[serde(rename = "projects:read")]
    ProjectsRead,
    /// Read and write the projects the user can write.
    #[serde(rename = "projects:write")]
    ProjectsWrite,
    /// Who the user is.
    #[serde(rename = "openid")]
    Openid,
    /// The user's email address.
    #[serde(rename = "email")]
    Email,
    /// The user's name.
    #[serde(rename = "profile")]
    Profile,
}

/// A set of OAuth scopes, written sorted and space-delimited.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct OAuthScopes(BTreeSet<OAuthScope>);

#[cfg(feature = "db")]
crate::typed_string!(OAuthScopes);

impl OAuthScopes {
    /// Whether these scopes permit what `scope` permits.
    pub fn permits(&self, scope: OAuthScope) -> bool {
        self.0.iter().any(|held| held.covers(scope))
    }

    pub fn iter(&self) -> impl Iterator<Item = OAuthScope> + '_ {
        self.0.iter().copied()
    }
}

impl OAuthScope {
    /// Whether this scope reaches projects, rather than asking who the user is.
    pub fn is_project(self) -> bool {
        match self {
            Self::ProjectsRead | Self::ProjectsWrite => true,
            Self::Openid | Self::Email | Self::Profile => false,
        }
    }

    /// Whether holding this scope permits what `scope` permits.
    fn covers(self, scope: Self) -> bool {
        match self {
            Self::ProjectsWrite => matches!(scope, Self::ProjectsRead | Self::ProjectsWrite),
            Self::ProjectsRead | Self::Openid | Self::Email | Self::Profile => scope == self,
        }
    }
}

impl fmt::Display for OAuthScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl FromStr for OAuthScope {
    type Err = ValidError;

    fn from_str(scope: &str) -> Result<Self, Self::Err> {
        match scope {
            PROJECTS_READ => Ok(Self::ProjectsRead),
            PROJECTS_WRITE => Ok(Self::ProjectsWrite),
            OPENID => Ok(Self::Openid),
            EMAIL => Ok(Self::Email),
            PROFILE => Ok(Self::Profile),
            _ => Err(ValidError::OAuthScope(scope.to_owned())),
        }
    }
}

impl AsRef<str> for OAuthScope {
    fn as_ref(&self) -> &str {
        match self {
            Self::ProjectsRead => PROJECTS_READ,
            Self::ProjectsWrite => PROJECTS_WRITE,
            Self::Openid => OPENID,
            Self::Email => EMAIL,
            Self::Profile => PROFILE,
        }
    }
}

// Sorted by name, so a written set reads the same whatever order it was built in.
impl Ord for OAuthScope {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

impl PartialOrd for OAuthScope {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for OAuthScopes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, scope) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(" ")?;
            }
            f.write_str(scope.as_ref())?;
        }
        Ok(())
    }
}

impl FromStr for OAuthScopes {
    type Err = ValidError;

    fn from_str(scopes: &str) -> Result<Self, Self::Err> {
        scopes
            .split(' ')
            .filter(|scope| !scope.is_empty())
            .map(str::parse)
            .collect()
    }
}

impl FromIterator<OAuthScope> for OAuthScopes {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = OAuthScope>,
    {
        Self(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{OAuthScope, OAuthScopes};

    #[test]
    fn write_permits_read() {
        let read: OAuthScopes = "projects:read".parse().unwrap();
        assert!(read.permits(OAuthScope::ProjectsRead));
        assert!(!read.permits(OAuthScope::ProjectsWrite));

        let write: OAuthScopes = "projects:write".parse().unwrap();
        assert!(write.permits(OAuthScope::ProjectsRead));
        assert!(write.permits(OAuthScope::ProjectsWrite));

        assert!(!OAuthScopes::default().permits(OAuthScope::ProjectsRead));
    }

    // An identity scope neither reaches projects nor comes with them.
    #[test]
    fn identity_scopes_permit_only_themselves() {
        let projects: OAuthScopes = "projects:read projects:write".parse().unwrap();
        let identity = [OAuthScope::Openid, OAuthScope::Email, OAuthScope::Profile];
        for scope in identity {
            let held: OAuthScopes = [scope].into_iter().collect();
            assert!(!held.permits(OAuthScope::ProjectsRead), "{scope}");
            assert!(!projects.permits(scope), "{scope}");
            for other in identity {
                assert_eq!(held.permits(other), scope == other, "{scope} {other}");
            }
        }
    }

    #[test]
    fn written_sorted_and_deduplicated() {
        let scopes: OAuthScopes = "projects:write projects:read projects:write"
            .parse()
            .unwrap();
        assert_eq!(scopes.to_string(), "projects:read projects:write");
    }
}
