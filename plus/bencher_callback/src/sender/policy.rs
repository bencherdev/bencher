use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Why an address is not on the public internet, so a callback never goes to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, derive_more::Display)]
pub enum AddressClass {
    /// `0.0.0.0/8` and `::`
    #[display("unspecified")]
    Unspecified,
    /// `10.0.0.0/8`, `172.16.0.0/12`, and `192.168.0.0/16`
    #[display("private")]
    Private,
    /// `100.64.0.0/10`, the carrier-grade NAT range
    #[display("shared")]
    Shared,
    /// `127.0.0.0/8` and `::1`
    #[display("loopback")]
    Loopback,
    /// `169.254.0.0/16` and `fe80::/10`, which hold the cloud metadata endpoints
    #[display("link_local")]
    LinkLocal,
    /// `192.0.0.0/24` and `2001::/23`
    #[display("protocol_assignments")]
    ProtocolAssignments,
    /// `192.0.2.0/24`, `198.51.100.0/24`, `203.0.113.0/24`, `2001:db8::/32`, and `3fff::/20`
    #[display("documentation")]
    Documentation,
    /// `198.18.0.0/15` and `2001:2::/48`
    #[display("benchmarking")]
    Benchmarking,
    /// `224.0.0.0/4` and `ff00::/8`
    #[display("multicast")]
    Multicast,
    /// `255.255.255.255`
    #[display("broadcast")]
    Broadcast,
    /// `fc00::/7`
    #[display("unique_local")]
    UniqueLocal,
    /// `100::/64`
    #[display("discard")]
    Discard,
    /// `240.0.0.0/4`, and any IPv6 address outside `2000::/3` with no other class
    #[display("reserved")]
    Reserved,
}

impl AddressClass {
    /// The class that keeps an address off the public internet, or `None` for a public address.
    pub fn of(address: IpAddr) -> Option<Self> {
        match address {
            IpAddr::V4(address) => Self::of_v4(address),
            IpAddr::V6(address) => Self::of_v6(address),
        }
    }

    fn of_v4(address: Ipv4Addr) -> Option<Self> {
        let class = if address.is_broadcast() {
            Self::Broadcast
        } else if address.is_private() {
            Self::Private
        } else if address.is_loopback() {
            Self::Loopback
        } else if address.is_link_local() {
            Self::LinkLocal
        } else if address.is_documentation() {
            Self::Documentation
        } else if address.is_multicast() {
            Self::Multicast
        } else if in_v4(address, Ipv4Addr::UNSPECIFIED, 8) {
            Self::Unspecified
        } else if in_v4(address, Ipv4Addr::new(100, 64, 0, 0), 10) {
            Self::Shared
        } else if in_v4(address, Ipv4Addr::new(192, 0, 0, 0), 24) {
            Self::ProtocolAssignments
        } else if in_v4(address, Ipv4Addr::new(198, 18, 0, 0), 15) {
            Self::Benchmarking
        } else if in_v4(address, Ipv4Addr::new(240, 0, 0, 0), 4) {
            Self::Reserved
        } else {
            return None;
        };
        Some(class)
    }

    fn of_v6(address: Ipv6Addr) -> Option<Self> {
        if let Some(embedded) = embedded_v4(address) {
            return Self::of_v4(embedded);
        }
        let class = if address.is_unspecified() {
            Self::Unspecified
        } else if address.is_loopback() {
            Self::Loopback
        } else if address.is_multicast() {
            Self::Multicast
        } else if address.is_unique_local() {
            Self::UniqueLocal
        } else if address.is_unicast_link_local() {
            Self::LinkLocal
        } else if in_v6(address, Ipv6Addr::new(0x100, 0, 0, 0, 0, 0, 0, 0), 64) {
            Self::Discard
        } else if in_v6(address, Ipv6Addr::new(0x2001, 2, 0, 0, 0, 0, 0, 0), 48) {
            Self::Benchmarking
        } else if in_v6(address, Ipv6Addr::new(0x2001, 0, 0, 0, 0, 0, 0, 0), 23) {
            Self::ProtocolAssignments
        } else if in_v6(address, Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 32)
            || in_v6(address, Ipv6Addr::new(0x3fff, 0, 0, 0, 0, 0, 0, 0), 20)
        {
            Self::Documentation
        } else if in_v6(address, Ipv6Addr::new(0x2000, 0, 0, 0, 0, 0, 0, 0), 3) {
            return None;
        } else {
            // Only `2000::/3` is allocated for global unicast.
            Self::Reserved
        };
        Some(class)
    }
}

/// The IPv4 address that an IPv6 address carries on to the IPv4 internet, if it carries one.
fn embedded_v4(address: Ipv6Addr) -> Option<Ipv4Addr> {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return Some(mapped);
    }
    // NAT64 in the well-known prefix, and in the `/96` that starts the local-use prefix
    // `64:ff9b:1::/48`. The rest of that `/48` places its IPv4 address wherever the local
    // translator says, so it has no class to take and falls outside `2000::/3`.
    if in_v6(address, Ipv6Addr::new(0x64, 0xff9b, 0, 0, 0, 0, 0, 0), 96)
        || in_v6(address, Ipv6Addr::new(0x64, 0xff9b, 1, 0, 0, 0, 0, 0), 96)
    {
        let [.., a, b, c, d] = address.octets();
        return Some(Ipv4Addr::new(a, b, c, d));
    }
    // 6to4
    if in_v6(address, Ipv6Addr::new(0x2002, 0, 0, 0, 0, 0, 0, 0), 16) {
        let [_, _, a, b, c, d, ..] = address.octets();
        return Some(Ipv4Addr::new(a, b, c, d));
    }
    None
}

fn in_v4(address: Ipv4Addr, network: Ipv4Addr, prefix: u32) -> bool {
    let mask = u32::MAX.checked_shl(u32::BITS - prefix).unwrap_or(0);
    (address.to_bits() & mask) == network.to_bits()
}

fn in_v6(address: Ipv6Addr, network: Ipv6Addr, prefix: u32) -> bool {
    let mask = u128::MAX.checked_shl(u128::BITS - prefix).unwrap_or(0);
    (address.to_bits() & mask) == network.to_bits()
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use super::AddressClass::{self, *};

    #[track_caller]
    fn assert_class(address: &str, expected: Option<AddressClass>) {
        let ip: IpAddr = address.parse().expect("a test address parses");
        assert_eq!(AddressClass::of(ip), expected, "the class of {address}");
    }

    #[track_caller]
    fn assert_range(start: &str, end: &str, class: AddressClass, outside: &[&str]) {
        assert_class(start, Some(class));
        assert_class(end, Some(class));
        for address in outside {
            assert_ne!(
                AddressClass::of(address.parse().expect("a test address parses")),
                Some(class),
                "{address} is outside the {class} range"
            );
        }
    }

    #[test]
    fn v4_this_network() {
        assert_range("0.0.0.0", "0.255.255.255", Unspecified, &["1.0.0.0"]);
        assert_class("1.0.0.0", None);
    }

    #[test]
    fn v4_private_10() {
        assert_range(
            "10.0.0.0",
            "10.255.255.255",
            Private,
            &["9.255.255.255", "11.0.0.0"],
        );
        assert_class("9.255.255.255", None);
        assert_class("11.0.0.0", None);
    }

    #[test]
    fn v4_shared() {
        assert_range(
            "100.64.0.0",
            "100.127.255.255",
            Shared,
            &["100.63.255.255", "100.128.0.0"],
        );
        assert_class("100.63.255.255", None);
        assert_class("100.128.0.0", None);
    }

    #[test]
    fn v4_loopback() {
        assert_range(
            "127.0.0.0",
            "127.255.255.255",
            Loopback,
            &["126.255.255.255", "128.0.0.0"],
        );
        assert_class("127.0.0.1", Some(Loopback));
        assert_class("126.255.255.255", None);
        assert_class("128.0.0.0", None);
    }

    #[test]
    fn v4_link_local() {
        assert_range(
            "169.254.0.0",
            "169.254.255.255",
            LinkLocal,
            &["169.253.255.255", "169.255.0.0"],
        );
        assert_class("169.254.169.254", Some(LinkLocal));
        assert_class("169.253.255.255", None);
        assert_class("169.255.0.0", None);
    }

    #[test]
    fn v4_private_172() {
        assert_range(
            "172.16.0.0",
            "172.31.255.255",
            Private,
            &["172.15.255.255", "172.32.0.0"],
        );
        assert_class("172.15.255.255", None);
        assert_class("172.32.0.0", None);
    }

    #[test]
    fn v4_protocol_assignments() {
        assert_range(
            "192.0.0.0",
            "192.0.0.255",
            ProtocolAssignments,
            &["191.255.255.255", "192.0.1.0"],
        );
        assert_class("191.255.255.255", None);
        assert_class("192.0.1.0", None);
    }

    #[test]
    fn v4_documentation_192() {
        assert_range(
            "192.0.2.0",
            "192.0.2.255",
            Documentation,
            &["192.0.1.255", "192.0.3.0"],
        );
        assert_class("192.0.1.255", None);
        assert_class("192.0.3.0", None);
    }

    #[test]
    fn v4_private_192() {
        assert_range(
            "192.168.0.0",
            "192.168.255.255",
            Private,
            &["192.167.255.255", "192.169.0.0"],
        );
        assert_class("192.167.255.255", None);
        assert_class("192.169.0.0", None);
    }

    #[test]
    fn v4_benchmarking() {
        assert_range(
            "198.18.0.0",
            "198.19.255.255",
            Benchmarking,
            &["198.17.255.255", "198.20.0.0"],
        );
        assert_class("198.17.255.255", None);
        assert_class("198.20.0.0", None);
    }

    #[test]
    fn v4_documentation_198() {
        assert_range(
            "198.51.100.0",
            "198.51.100.255",
            Documentation,
            &["198.51.99.255", "198.51.101.0"],
        );
        assert_class("198.51.99.255", None);
        assert_class("198.51.101.0", None);
    }

    #[test]
    fn v4_documentation_203() {
        assert_range(
            "203.0.113.0",
            "203.0.113.255",
            Documentation,
            &["203.0.112.255", "203.0.114.0"],
        );
        assert_class("203.0.112.255", None);
        assert_class("203.0.114.0", None);
    }

    #[test]
    fn v4_multicast() {
        assert_range(
            "224.0.0.0",
            "239.255.255.255",
            Multicast,
            &["223.255.255.255", "240.0.0.0"],
        );
        assert_class("223.255.255.255", None);
    }

    #[test]
    fn v4_reserved() {
        assert_range(
            "240.0.0.0",
            "255.255.255.254",
            Reserved,
            &["239.255.255.255", "255.255.255.255"],
        );
    }

    #[test]
    fn v4_broadcast() {
        assert_class("255.255.255.255", Some(Broadcast));
        assert_class("255.255.255.254", Some(Reserved));
    }

    #[test]
    fn v4_public() {
        for address in ["1.1.1.1", "8.8.8.8", "93.184.216.34", "140.82.112.3"] {
            assert_class(address, None);
        }
    }

    #[test]
    fn v6_unspecified() {
        assert_class("::", Some(Unspecified));
    }

    #[test]
    fn v6_loopback() {
        assert_class("::1", Some(Loopback));
        assert_ne!(
            AddressClass::of("::2".parse().expect("a test address parses")),
            Some(Loopback),
            "::2 is not loopback"
        );
    }

    #[test]
    fn v6_ipv4_mapped() {
        assert_class("::ffff:0.0.0.0", Some(Unspecified));
        assert_class("::ffff:255.255.255.255", Some(Broadcast));
        assert_class("::ffff:10.0.0.1", Some(Private));
        assert_class("::ffff:127.0.0.1", Some(Loopback));
        assert_class("::ffff:169.254.169.254", Some(LinkLocal));
        assert_class("::ffff:8.8.8.8", None);
        // Just outside `::ffff:0:0/96`: nothing is unwrapped, and nothing outside `2000::/3` is public.
        assert_class("::fffe:ffff:ffff", Some(Reserved));
        assert_class("::1:0:0:0", Some(Reserved));
    }

    #[test]
    fn v6_nat64_well_known() {
        assert_class("64:ff9b::", Some(Unspecified));
        assert_class("64:ff9b::ffff:ffff", Some(Broadcast));
        assert_class("64:ff9b::10.0.0.1", Some(Private));
        assert_class("64:ff9b::a9fe:a9fe", Some(LinkLocal));
        assert_class("64:ff9b::8.8.8.8", None);
        // Just outside `64:ff9b::/96`.
        assert_class("64:ff9b::1:808:808", Some(Reserved));
        assert_class("64:ff9a:ffff:ffff:ffff:ffff:808:808", Some(Reserved));
    }

    #[test]
    fn v6_nat64_local_use() {
        assert_class("64:ff9b:1::", Some(Unspecified));
        assert_class("64:ff9b:1::10.0.0.1", Some(Private));
        assert_class("64:ff9b:1::127.0.0.1", Some(Loopback));
        assert_class("64:ff9b:1::8.8.8.8", None);
        // The rest of `64:ff9b:1::/48` embeds its IPv4 address where the translator says, so
        // none of it is public.
        assert_class("64:ff9b:1:0:8:808:800:0", Some(Reserved));
        assert_class("64:ff9b:1:ffff:ffff:ffff:ffff:ffff", Some(Reserved));
        // Just outside `64:ff9b:1::/48`.
        assert_class("64:ff9b:2::808:808", Some(Reserved));
    }

    #[test]
    fn v6_discard() {
        assert_range(
            "100::",
            "100::ffff:ffff:ffff:ffff",
            Discard,
            &["100:0:0:1::", "ff:ffff:ffff:ffff:ffff:ffff:ffff:ffff"],
        );
    }

    #[test]
    fn v6_documentation() {
        assert_range(
            "2001:db8::",
            "2001:db8:ffff:ffff:ffff:ffff:ffff:ffff",
            Documentation,
            &["2001:db7:ffff:ffff:ffff:ffff:ffff:ffff", "2001:db9::"],
        );
        assert_class("2001:db7:ffff:ffff:ffff:ffff:ffff:ffff", None);
        assert_class("2001:db9::", None);
        assert_range(
            "3fff::",
            "3fff:fff:ffff:ffff:ffff:ffff:ffff:ffff",
            Documentation,
            &["3ffe:ffff:ffff:ffff:ffff:ffff:ffff:ffff", "3fff:1000::"],
        );
        assert_class("3ffe:ffff:ffff:ffff:ffff:ffff:ffff:ffff", None);
        assert_class("3fff:1000::", None);
    }

    #[test]
    fn v6_unique_local() {
        assert_range(
            "fc00::",
            "fdff:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
            UniqueLocal,
            &["fbff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", "fe00::"],
        );
        assert_class("fd00:ec2::254", Some(UniqueLocal));
    }

    #[test]
    fn v6_link_local() {
        assert_range(
            "fe80::",
            "febf:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
            LinkLocal,
            &["fe7f:ffff:ffff:ffff:ffff:ffff:ffff:ffff", "fec0::"],
        );
        // The deprecated site-local range is outside `2000::/3`.
        assert_class("fec0::1", Some(Reserved));
    }

    #[test]
    fn v6_multicast() {
        assert_range(
            "ff00::",
            "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
            Multicast,
            &["feff:ffff:ffff:ffff:ffff:ffff:ffff:ffff"],
        );
    }

    #[test]
    fn v6_six_to_four() {
        assert_class("2002::", Some(Unspecified));
        assert_class("2002:ffff:ffff::", Some(Broadcast));
        assert_class("2002:a00:1::", Some(Private));
        assert_class("2002:7f00:1:ffff:ffff:ffff:ffff:ffff", Some(Loopback));
        assert_class("2002:a9fe:a9fe::1", Some(LinkLocal));
        assert_class("2002:808:808::", None);
        // Just outside `2002::/16`: ordinary global unicast, nothing unwrapped.
        assert_class("2001:ffff:a00:1::", None);
        assert_class("2003::a00:1", None);
    }

    #[test]
    fn v6_protocol_assignments() {
        assert_range(
            "2001::",
            "2001:1ff:ffff:ffff:ffff:ffff:ffff:ffff",
            ProtocolAssignments,
            &["2000:ffff:ffff:ffff:ffff:ffff:ffff:ffff", "2001:200::"],
        );
        assert_class("2000:ffff:ffff:ffff:ffff:ffff:ffff:ffff", None);
        assert_class("2001:200::", None);
        // Teredo
        assert_class(
            "2001:0:4136:e378:8000:63bf:3fff:fdd2",
            Some(ProtocolAssignments),
        );
    }

    #[test]
    fn v6_benchmarking() {
        assert_range(
            "2001:2::",
            "2001:2:0:ffff:ffff:ffff:ffff:ffff",
            Benchmarking,
            &["2001:1:ffff:ffff:ffff:ffff:ffff:ffff", "2001:2:1::"],
        );
    }

    #[test]
    fn v6_outside_global_unicast() {
        // IPv4-compatible, deprecated
        assert_class("::10.0.0.1", Some(Reserved));
        assert_class("::8.8.8.8", Some(Reserved));
        // IPv4-translated
        assert_class("::ffff:0:808:808", Some(Reserved));
        // Just outside `2000::/3` at both ends.
        assert_class("1fff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", Some(Reserved));
        assert_class("4000::", Some(Reserved));
        assert_class("5f00::1", Some(Reserved));
    }

    #[test]
    fn v6_public() {
        for address in [
            "2000::",
            "2606:4700:4700::1111",
            "2001:4860:4860::8888",
            "2a00:1450:4001:80b::200e",
            "3fff:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
        ] {
            assert_class(address, None);
        }
    }

    // Every IPv6 form that carries an IPv4 address onward takes the class of that address.
    #[test]
    fn v6_embedded_forms_take_the_ipv4_class() {
        let cases = [
            (Ipv4Addr::UNSPECIFIED, Some(Unspecified)),
            (Ipv4Addr::new(10, 1, 2, 3), Some(Private)),
            (Ipv4Addr::new(100, 64, 0, 1), Some(Shared)),
            (Ipv4Addr::LOCALHOST, Some(Loopback)),
            (Ipv4Addr::new(169, 254, 169, 254), Some(LinkLocal)),
            (Ipv4Addr::new(172, 16, 0, 1), Some(Private)),
            (Ipv4Addr::new(192, 0, 0, 1), Some(ProtocolAssignments)),
            (Ipv4Addr::new(192, 0, 2, 1), Some(Documentation)),
            (Ipv4Addr::new(192, 168, 1, 1), Some(Private)),
            (Ipv4Addr::new(198, 18, 0, 1), Some(Benchmarking)),
            (Ipv4Addr::new(198, 51, 100, 1), Some(Documentation)),
            (Ipv4Addr::new(203, 0, 113, 1), Some(Documentation)),
            (Ipv4Addr::new(224, 0, 0, 1), Some(Multicast)),
            (Ipv4Addr::new(240, 0, 0, 1), Some(Reserved)),
            (Ipv4Addr::BROADCAST, Some(Broadcast)),
            (Ipv4Addr::new(1, 1, 1, 1), None),
            (Ipv4Addr::new(8, 8, 8, 8), None),
            (Ipv4Addr::new(140, 82, 112, 3), None),
        ];
        for (v4, expected) in cases {
            let bits = u128::from(v4.to_bits());
            let forms = [
                v4.to_ipv6_mapped(),
                Ipv6Addr::from_bits(Ipv6Addr::new(0x64, 0xff9b, 0, 0, 0, 0, 0, 0).to_bits() | bits),
                Ipv6Addr::from_bits(Ipv6Addr::new(0x64, 0xff9b, 1, 0, 0, 0, 0, 0).to_bits() | bits),
                Ipv6Addr::from_bits(
                    Ipv6Addr::new(0x2002, 0, 0, 0, 0, 0, 0, 1).to_bits() | (bits << 80),
                ),
            ];
            assert_eq!(AddressClass::of(IpAddr::V4(v4)), expected, "{v4}");
            for form in forms {
                assert_eq!(
                    AddressClass::of(IpAddr::V6(form)),
                    expected,
                    "{form} carries {v4}"
                );
            }
        }
    }
}
