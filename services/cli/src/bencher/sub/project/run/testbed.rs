use bencher_json::{project::testbed::TESTBED_LOCALHOST_STR, NameId};

use super::BENCHER_TESTBED;

#[derive(Debug, Clone)]
pub struct Testbed(pub NameId);

#[derive(thiserror::Error, Debug)]
pub enum TestbedError {
    #[error("Failed to parse UUID, slug, or name for the testbed: {0}")]
    ParseTestbed(bencher_json::ValidError),
}

impl TryFrom<Option<NameId>> for Testbed {
    type Error = TestbedError;

    fn try_from(testbed: Option<NameId>) -> Result<Self, Self::Error> {
        Ok(Testbed(if let Some(testbed) = testbed {
            testbed
        } else if let Ok(env_testbed) = std::env::var(BENCHER_TESTBED) {
            env_testbed
                .as_str()
                .parse()
                .map_err(TestbedError::ParseTestbed)?
        } else {
            TESTBED_LOCALHOST_STR
                .parse()
                .map_err(TestbedError::ParseTestbed)?
        }))
    }
}

impl From<Testbed> for bencher_client::types::NameId {
    fn from(testbed: Testbed) -> Self {
        testbed.0.into()
    }
}
