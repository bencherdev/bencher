use bencher_json::{Entitlements, OrganizationUuid, PlanLevel, Secret, PROD_BENCHER_URL_STR};
use bencher_license::{BillingCycle, Licensor};

use crate::parser::{TaskBillingCycle, TaskLicenseGenerate};

#[derive(Debug)]
pub struct Generate {
    organization: OrganizationUuid,
    pem: String,
    billing_cycle: BillingCycle,
    level: PlanLevel,
    entitlements: Entitlements,
}

impl TryFrom<TaskLicenseGenerate> for Generate {
    type Error = anyhow::Error;

    fn try_from(generate: TaskLicenseGenerate) -> Result<Self, Self::Error> {
        let TaskLicenseGenerate {
            organization,
            pem,
            cycle,
            level,
            entitlements,
        } = generate;
        let billing_cycle = match cycle {
            TaskBillingCycle::Monthly => BillingCycle::Monthly,
            TaskBillingCycle::Annual => BillingCycle::Annual,
        };
        Ok(Self {
            organization,
            pem,
            billing_cycle,
            level,
            entitlements,
        })
    }
}

impl Generate {
    pub fn exec(&self) -> anyhow::Result<()> {
        let pem: Secret = self.pem.parse()?;
        let licensor = Licensor::bencher_cloud(&pem)?;
        let license = licensor.new_license(
            bencher_license::Audience::default(),
            self.billing_cycle,
            self.organization,
            self.level,
            self.entitlements,
            Some(PROD_BENCHER_URL_STR.to_owned()),
        )?;
        Ok(println!("{license}"))
    }
}
