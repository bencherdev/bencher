use bencher_json::{DateTime, Entitlements, OrganizationUuid, PROD_BENCHER_URL_STR, Secret};
use bencher_license::Licensor;

use crate::parser::TaskLicenseGenerate;

#[derive(Debug)]
pub struct Generate {
    organization: OrganizationUuid,
    pem: String,
    series: Entitlements,
    term_end: DateTime,
}

impl TryFrom<TaskLicenseGenerate> for Generate {
    type Error = anyhow::Error;

    fn try_from(generate: TaskLicenseGenerate) -> Result<Self, Self::Error> {
        let TaskLicenseGenerate {
            organization,
            pem,
            series,
            term_end,
        } = generate;
        Ok(Self {
            organization,
            pem,
            series,
            term_end,
        })
    }
}

impl Generate {
    pub fn exec(&self) -> anyhow::Result<()> {
        let pem: Secret = self.pem.parse()?;
        let licensor = Licensor::bencher_cloud(&pem)?;
        let license = licensor.new_license_with_issuer(
            self.organization,
            self.series,
            DateTime::now(),
            self.term_end,
            PROD_BENCHER_URL_STR,
        )?;
        println!("{license}");
        Ok(())
    }
}
