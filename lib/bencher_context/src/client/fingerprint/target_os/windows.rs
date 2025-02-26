use uuid::Uuid;

impl crate::Fingerprint {
    pub fn new() -> Option<Self> {
        windows::System::Profile::SystemManufacturers::SmbiosInformation::SerialNumber()
            .ok()
            .as_ref()
            .and_then(|uuid| Uuid::parse_str(&uuid.to_string().trim()).ok())
            .map(Self)
    }
}
