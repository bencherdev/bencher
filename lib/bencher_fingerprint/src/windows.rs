use uuid::Uuid;

impl crate::Fingerprint {
    pub fn new() -> Option<Self> {
        windows::System::Profile::SystemManufacturers::SmbiosInformation::SerialNumber()
            .ok()
            .as_ref()
            .and_then(|hstring| Uuid::parse_str(&hstring.to_string()).ok())
            .map(|uuid| uuid.as_u128())
            .map(Self)
    }
}
