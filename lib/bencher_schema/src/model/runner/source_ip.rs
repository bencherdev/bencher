//! `SourceIp` newtype for validated IP address storage in the database.

use std::{fmt, net::IpAddr, str::FromStr};

use diesel::{
    backend::Backend,
    deserialize::{self, FromSql},
    serialize::{self, IsNull, Output, ToSql},
    sql_types::Text,
    sqlite::Sqlite,
};

/// A validated IP address stored as TEXT in `SQLite`.
///
/// Provides type safety for IP addresses while storing them as strings in the database.
/// Supports both IPv4 and IPv6 addresses.
#[derive(Debug, Clone, PartialEq, Eq, Hash, diesel::AsExpression, diesel::FromSqlRow)]
#[diesel(sql_type = Text)]
pub struct SourceIp(IpAddr);

impl SourceIp {
    /// Create a new `SourceIp` from an `IpAddr`.
    pub fn new(ip: IpAddr) -> Self {
        Self(ip)
    }

    /// Get the underlying `IpAddr`.
    pub fn inner(&self) -> IpAddr {
        self.0
    }
}

impl fmt::Display for SourceIp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<IpAddr> for SourceIp {
    fn from(ip: IpAddr) -> Self {
        Self(ip)
    }
}

impl From<SourceIp> for IpAddr {
    fn from(source_ip: SourceIp) -> Self {
        source_ip.0
    }
}

impl FromStr for SourceIp {
    type Err = std::net::AddrParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<IpAddr>().map(Self)
    }
}

impl ToSql<Text, Sqlite> for SourceIp {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
        out.set_value(self.0.to_string());
        Ok(IsNull::No)
    }
}

impl<DB> FromSql<Text, DB> for SourceIp
where
    DB: Backend,
    String: FromSql<Text, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> deserialize::Result<Self> {
        let s = <String as FromSql<Text, DB>>::from_sql(bytes)?;
        s.parse::<IpAddr>()
            .map(Self)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use super::*;

    #[test]
    fn ipv4_roundtrip() {
        let ip = SourceIp::new(IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(ip.to_string(), "127.0.0.1");
        assert_eq!("127.0.0.1".parse::<SourceIp>().unwrap(), ip);
    }

    #[test]
    fn ipv6_roundtrip() {
        let ip = SourceIp::new(IpAddr::V6(Ipv6Addr::LOCALHOST));
        assert_eq!(ip.to_string(), "::1");
        assert_eq!("::1".parse::<SourceIp>().unwrap(), ip);
    }

    #[test]
    fn invalid_ip() {
        assert!("not-an-ip".parse::<SourceIp>().is_err());
    }
}
