use uuid::Uuid;

impl crate::Fingerprint {
    pub fn new() -> Option<Self> {
        windows::System::Profile::SystemManufacturers::SmbiosInformation::SerialNumber()
            .ok()
            .as_ref()
            .and_then(|hstring| {
                let uuid = hstring.to_string();
                println!("{uuid}");
                println!("{}", uuid.trim());
                Uuid::parse_str(&hstring.to_string().trim()).ok()
            })
            .map(Self)
    }
}
