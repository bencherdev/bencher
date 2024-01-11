use bencher_json::{Jwt, Secret, PROD_BENCHER_URL_STR};
use bencher_license::{Licensor, PublicKey};

use crate::parser::TaskLicenseValidate;

#[derive(Debug)]
pub struct Validate {
    license: Jwt,
    pem: String,
}

impl TryFrom<TaskLicenseValidate> for Validate {
    type Error = anyhow::Error;

    fn try_from(validate: TaskLicenseValidate) -> Result<Self, Self::Error> {
        let TaskLicenseValidate { license, pem } = validate;
        Ok(Self { license, pem })
    }
}

impl Validate {
    pub fn exec(&self) -> anyhow::Result<()> {
        let pem: Secret = self.pem.parse()?;
        let licensor = Licensor::bencher_cloud_with_public_key(&pem, Some(PublicKey::Live))?;
        let claims = licensor.validate_with_issuer(&self.license, PROD_BENCHER_URL_STR)?;
        Ok(print!("{claims:#?}"))
    }
}
