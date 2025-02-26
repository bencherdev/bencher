use uuid::Uuid;

use crate::client::platform::OperatingSystem;

impl crate::Fingerprint {
    pub fn current() -> Option<Self> {
        windows::System::Profile::SystemManufacturers::SmbiosInformation::SerialNumber()
            .ok()
            .as_ref()
            .and_then(|uuid| Uuid::parse_str(&uuid.to_string().trim()).ok())
            .map(Self)
    }
}

impl OperatingSystem {
    pub fn current() -> Self {
        Self::Windows
    }
}
