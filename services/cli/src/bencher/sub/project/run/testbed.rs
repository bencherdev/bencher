use bencher_client::types::JsonNewTestbed;
use bencher_json::{
    project::testbed::TESTBED_LOCALHOST_STR, JsonUuid, JsonUuids, NameId, NameIdKind, ResourceId,
    ResourceName, TestbedUuid,
};

use crate::bencher::backend::AuthBackend;

use super::BENCHER_TESTBED;

#[derive(Debug)]
pub struct Testbed(pub NameId);

#[derive(thiserror::Error, Debug)]
pub enum TestbedError {
    #[error("Failed to parse UUID, slug, or name for the testbed: {0}")]
    ParseTestbed(bencher_json::ValidError),
    #[error(
        "No testbeds were found with name \"{testbed_name}\" in project \"{project}\". Exactly one was expected.\nDoes it exist? Testbeds need to already exist.\nSee: https://bencher.dev/docs/explanation/bencher-run/#--testbed-testbed"
    )]
    NoTestbeds {
        project: String,
        testbed_name: String,
    },
    #[error(
        "{count} testbeds were found with name \"{testbed_name}\" in project \"{project}\"! Exactly one was expected.\nThis is likely a bug. Please report it here: https://github.com/bencherdev/bencher/issues"
    )]
    MultipleTestbeds {
        project: String,
        testbed_name: String,
        count: usize,
    },
    #[error("Failed to get testbed: {0}\nDoes it exist? Testbeds must already exist when using a UUID.\nSee: https://bencher.dev/docs/explanation/bencher-run/#--testbed-testbed")]
    GetTestbed(crate::bencher::BackendError),
    #[error("Failed to query testbeds: {0}")]
    GetTestbeds(crate::bencher::BackendError),
    #[error("Failed to create new testbed: {0}")]
    CreateTestbed(crate::bencher::BackendError),
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

impl Testbed {
    pub async fn get(
        &self,
        project: &ResourceId,
        dry_run: bool,
        backend: &AuthBackend,
    ) -> Result<NameId, TestbedError> {
        // Check to make sure that the testbed exists before running the benchmarks
        match (&self.0).try_into().map_err(TestbedError::ParseTestbed)? {
            NameIdKind::Uuid(uuid) => {
                if !dry_run {
                    get_testbed(project, &uuid.into(), backend).await?;
                }
            },
            NameIdKind::Slug(slug) => {
                if !dry_run {
                    match get_testbed(project, &slug.clone().into(), backend).await {
                        Ok(_) => {},
                        Err(TestbedError::GetTestbed(_)) => {
                            create_testbed(project, &slug.into(), backend).await?;
                        },
                        Err(e) => return Err(e),
                    }
                }
            },
            NameIdKind::Name(name) => {
                let testbed_name: ResourceName = name;
                if !dry_run {
                    match get_testbed_by_name(project, &testbed_name, backend).await {
                        Ok(_) => {},
                        Err(TestbedError::NoTestbeds { .. }) => {
                            create_testbed(project, &testbed_name, backend).await?;
                        },
                        Err(e) => return Err(e),
                    }
                }
            },
        }
        Ok(self.0.clone())
    }
}

async fn get_testbed(
    project: &ResourceId,
    testbed: &ResourceId,
    backend: &AuthBackend,
) -> Result<TestbedUuid, TestbedError> {
    // Use `JsonUuid` to future proof against breaking changes
    let json_testbed: JsonUuid = backend
        .send_with(|client| async move {
            client
                .proj_testbed_get()
                .project(project.clone())
                .testbed(testbed.clone())
                .send()
                .await
        })
        .await
        .map_err(TestbedError::GetTestbed)?;

    Ok(json_testbed.uuid.into())
}

async fn get_testbed_by_name(
    project: &ResourceId,
    testbed_name: &ResourceName,
    backend: &AuthBackend,
) -> Result<Option<TestbedUuid>, TestbedError> {
    // Use `JsonUuids` to future proof against breaking changes
    let json_testbeds: JsonUuids = backend
        .send_with(|client| async move {
            client
                .proj_testbeds_get()
                .project(project.clone())
                .name(testbed_name.clone())
                .send()
                .await
        })
        .await
        .map_err(TestbedError::GetTestbeds)?;

    let mut json_testbeds = json_testbeds.into_inner();
    let testbed_count = json_testbeds.len();
    if let Some(testbed) = json_testbeds.pop() {
        if testbed_count == 1 {
            Ok(Some(testbed.uuid.into()))
        } else {
            Err(TestbedError::MultipleTestbeds {
                project: project.to_string(),
                testbed_name: testbed_name.as_ref().into(),
                count: testbed_count,
            })
        }
    } else {
        Err(TestbedError::NoTestbeds {
            project: project.to_string(),
            testbed_name: testbed_name.as_ref().into(),
        })
    }
}

async fn create_testbed(
    project: &ResourceId,
    testbed_name: &ResourceName,
    backend: &AuthBackend,
) -> Result<TestbedUuid, TestbedError> {
    let new_testbed = &JsonNewTestbed {
        name: testbed_name.clone().into(),
        slug: None,
        soft: Some(true),
    };
    // Use `JsonUuid` to future proof against breaking changes
    let json_testbed: JsonUuid = backend
        .send_with(|client| async move {
            client
                .proj_testbed_post()
                .project(project.clone())
                .body(new_testbed.clone())
                .send()
                .await
        })
        .await
        .map_err(TestbedError::CreateTestbed)?;

    Ok(json_testbed.uuid.into())
}
