use bencher_valid::Jwt;

pub struct License(Jwt);

impl AsRef<str> for License {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl From<License> for Jwt {
    fn from(license: License) -> Self {
        license.0
    }
}
