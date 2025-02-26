use std::fs;

use uuid::Uuid;

const HEX_BASE: u32 = 16;

impl crate::Fingerprint {
    pub fn new() -> Option<Self> {
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
