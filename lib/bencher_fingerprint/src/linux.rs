use std::fs;

const HEX_BASE: u32 = 16;

impl crate::Fingerprint {
    pub fn new() -> Option<Self> {
        fs::read_to_string("/etc/machine-id")
            .ok()
            .and_then(|id| u128::from_str_radix(&id, HEX_BASE).ok())
            .map(Self)
    }
}
