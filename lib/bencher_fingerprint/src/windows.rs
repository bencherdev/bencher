use uuid::Uuid;

impl crate::Fingerprint {
    pub fn new() -> Option<Self> {
        windows::System::Profile::SystemManufacturers::SmbiosInformation::SerialNumber()
            .ok()
            .as_ref()
            .and_then(|uuid| Uuid::parse_str(uuid).ok())
            .map(|uuid| uuid.as_u128())
            .map(Self)
    }
}
