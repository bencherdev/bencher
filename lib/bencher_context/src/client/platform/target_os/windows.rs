use std::cmp;

use uuid::Uuid;
use windows::{
    core::PCWSTR,
    System::Profile::SystemManufacturers::SmbiosInformation,
    Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_LOCAL_MACHINE, KEY_READ,
    },
};

use crate::client::platform::OperatingSystem;

impl crate::Fingerprint {
    pub fn current() -> Option<Self> {
        serial_number().or_else(digital_product_id).map(Self)
    }
}

fn serial_number() -> Option<Uuid> {
    SmbiosInformation::SerialNumber()
        .ok()
        .as_ref()
        .and_then(|uuid| Uuid::parse_str(&uuid.to_string().trim()).ok())
}

fn digital_product_id() -> Option<Uuid> {
    let key = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\0"
        .encode_utf16()
        .collect::<Vec<u16>>();
    let sub_key = PCWSTR::from_raw(key.as_ptr());
    let hkey = RegOpenKeyExW(HKEY_LOCAL_MACHINE, sub_key, 0, KEY_READ).ok()?;
    let value = "DigitalProductId\0".encode_utf16().collect::<Vec<u16>>();
    let value_name = PCWSTR::from_raw(value.as_ptr());
    let mut data = vec![0u8; 256];
    let mut data_size = data.len() as u32;
    RegQueryValueExW(
        hkey,
        value_name,
        None,
        None,
        Some(&mut data),
        Some(&mut data_size),
    )
    .ok()?;
    RegCloseKey(hkey);

    let digital_product_id = data
        .into_iter()
        .take(cmp::min(data_size as usize, size_of::<uuid::Bytes>()))
        .collect::<Vec<u8>>();
    digital_product_id.try_into().ok().map(Uuid::from_bytes)
}

impl OperatingSystem {
    pub fn current() -> Self {
        Self::Windows
    }
}
