use std::net::{IpAddr, Ipv6Addr};

/// The upper 64 bits of an IPv6 address: the block one subscriber holds.
const IPV6_SUBSCRIBER: u128 = u128::MAX << 64;

/// A reader's address as every per-IP limit keys it: an IPv4 address as itself, an IPv4-mapped
/// IPv6 address as its IPv4 address, and any other IPv6 address by its /64, so that one
/// subscriber's block is one reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(into = "IpAddr", from = "IpAddr")]
pub(super) struct IpKey(IpAddr);

impl From<IpAddr> for IpKey {
    fn from(ip: IpAddr) -> Self {
        Self(match ip {
            IpAddr::V4(_) => ip,
            IpAddr::V6(ipv6) => ipv6.to_ipv4_mapped().map_or_else(
                || IpAddr::V6(Ipv6Addr::from_bits(ipv6.to_bits() & IPV6_SUBSCRIBER)),
                IpAddr::V4,
            ),
        })
    }
}

impl From<IpKey> for IpAddr {
    fn from(key: IpKey) -> Self {
        key.0
    }
}
