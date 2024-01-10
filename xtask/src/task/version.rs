use bencher_api::API_VERSION;

use crate::parser::TaskVersion;

#[derive(Debug)]
pub struct Version {}

impl TryFrom<TaskVersion> for Version {
    type Error = anyhow::Error;

    fn try_from(version: TaskVersion) -> Result<Self, Self::Error> {
        let TaskVersion {} = version;
        Ok(Self {})
    }
}

impl Version {
    #[allow(clippy::unnecessary_wraps)]
    pub fn exec(&self) -> anyhow::Result<()> {
        Ok(println!("{API_VERSION}"))
    }
}
