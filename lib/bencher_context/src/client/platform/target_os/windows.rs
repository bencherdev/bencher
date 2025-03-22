use core::ffi::c_void;

use uuid::Uuid;
use windows::{
    core::PCWSTR,
    System::Profile::SystemManufacturers::SmbiosInformation,
    Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_ANY},
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
    let sub_key_bytes = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\0"
        .encode_utf16()
        .collect::<Vec<u16>>();
    let sub_key = PCWSTR::from_raw(sub_key_bytes.as_ptr());

    let value_bytes = "DigitalProductId\0".encode_utf16().collect::<Vec<u16>>();
    let value = PCWSTR::from_raw(value_bytes.as_ptr());

    let mut data = [0u8; 256];
    let mut data_size = data.len() as u32;
    // Safety: The accuracy of the data returned by `RegGetValueW` is not of any importance,
    // rather the consistency of the data is what is important.
    // https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-reggetvaluew
    #[allow(unsafe_code)]
    unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            sub_key,
            value,
            RRF_RT_ANY,
            None,
            Some(data.as_mut_ptr() as *mut c_void),
            Some(&mut data_size),
        )
        .ok()
        .ok()?;
    }

    // There appear to be quite a few zeroed out bytes at the beginning of the digital product ID.
    // In order to ensure as much entropy as possible,
    // we'll just shift all of the bits dropped by the leading bytes as needed.
    let digital_product_id = data
        .into_iter()
        .take(data_size as usize)
        .fold(0u128, |acc, byte| (acc << 8) | u128::from(byte));
    Some(Uuid::from_u128(digital_product_id))
}

impl OperatingSystem {
    #[allow(clippy::unnecessary_wraps)]
    pub fn current() -> Option<Self> {
        Some(Self::Windows)
    }
}
