use std::fs;

use uuid::Uuid;

use crate::client::platform::OperatingSystem;

const HEX_BASE: u32 = 16;

impl crate::Fingerprint {
    pub fn current() -> Option<Self> {
        parse_machine_id("/var/lib/dbus/machine-id")
            .or_else(|| parse_machine_id("/etc/machine-id"))
            .map(Self)
    }
}

fn parse_machine_id(path: &str) -> Option<Uuid> {
    fs::read_to_string(path)
        .ok()
        .and_then(|id| u128::from_str_radix(id.trim(), HEX_BASE).ok())
        .map(Uuid::from_u128)
}

impl OperatingSystem {
    #[allow(clippy::unnecessary_wraps)]
    pub fn current() -> Option<Self> {
        Some(Self::Linux)
    }
}
