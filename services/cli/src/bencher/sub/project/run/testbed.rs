use bencher_client::{types::JsonNewTestbed, ClientError, ErrorResponse};
use bencher_json::{
    project::testbed::TESTBED_LOCALHOST_STR, JsonTestbed, JsonTestbeds, NameId, NameIdKind,
    ResourceId, ResourceName, Slug,
};

use crate::{bencher::backend::AuthBackend, cli_println_quietable};

use super::BENCHER_TESTBED;

#[derive(Debug)]
pub struct Testbed(pub NameId);

#[derive(thiserror::Error, Debug)]
pub enum TestbedError {
    #[error("Failed to parse UUID, slug, or name for the testbed: {0}")]
    ParseTestbed(bencher_json::ValidError),
    #[error("Failed to get testbed with UUID: {0}\nDoes it exist? Testbed must already exist when using `--testbed` or `BENCHER_TESTBED` with a UUID.\nSee: https://bencher.dev/docs/explanation/bencher-run/#--testbed-testbed")]
    GetTestbedUuid(crate::BackendError),
    #[error("Failed to get testbed with slug: {0}")]
    GetTestbedSlug(crate::BackendError),
    #[error("Failed to query testbeds: {0}")]
    GetTestbeds(crate::bencher::BackendError),
    #[error(
        "{count} testbeds were found with name \"{testbed_name}\" in project \"{project}\"! Exactly one was expected.\nThis is likely a bug. Please report it here: https://github.com/bencherdev/bencher/issues"
    )]
    MultipleTestbeds {
        project: String,
        testbed_name: String,
        count: usize,
    },
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
        log: bool,
        backend: &AuthBackend,
    ) -> Result<NameId, TestbedError> {
        if !dry_run {
            // Check to make sure that the testbed exists before running the benchmarks
            self.exists_or_create(project, log, backend).await?;
        }
        Ok(self.0.clone())
    }

    async fn exists_or_create(
        &self,
        project: &ResourceId,
        log: bool,
        backend: &AuthBackend,
    ) -> Result<(), TestbedError> {
        match (&self.0).try_into().map_err(TestbedError::ParseTestbed)? {
            NameIdKind::Uuid(uuid) => {
                get_testbed(project, &uuid.into(), backend)
                    .await
                    .map_err(TestbedError::GetTestbedUuid)?;
            },
            NameIdKind::Slug(slug) => {
                match get_testbed(project, &slug.clone().into(), backend).await {
                    Ok(_) => {},
                    Err(crate::BackendError::Client(ClientError::ErrorResponse(
                        ErrorResponse {
                            status: reqwest::StatusCode::NOT_FOUND,
                            ..
                        },
                    ))) => {
                        cli_println_quietable!(
                            log,
                            "Failed to find testbed with slug \"{slug}\" in project \"{project}\"."
                        );
                        create_testbed(project, slug.clone().into(), Some(slug), log, backend)
                            .await?;
                    },
                    Err(e) => return Err(TestbedError::GetTestbedSlug(e)),
                }
            },
            NameIdKind::Name(name) => match get_testbed_by_name(project, &name, backend).await {
                Ok(Some(_)) => {},
                Ok(None) => {
                    cli_println_quietable!(
                        log,
                        "Failed to find testbed with name \"{name}\" in project \"{project}\"."
                    );
                    create_testbed(project, name, None, log, backend).await?;
                },
                Err(e) => return Err(e),
            },
        }
        Ok(())
    }
}

async fn get_testbed(
    project: &ResourceId,
    testbed: &ResourceId,
    backend: &AuthBackend,
) -> Result<JsonTestbed, crate::BackendError> {
    backend
        .send_with(|client| async move {
            client
                .proj_testbed_get()
                .project(project.clone())
                .testbed(testbed.clone())
                .send()
                .await
        })
        .await
}

async fn get_testbed_by_name(
    project: &ResourceId,
    testbed_name: &ResourceName,
    backend: &AuthBackend,
) -> Result<Option<JsonTestbed>, TestbedError> {
    let json_testbeds: JsonTestbeds = backend
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
            Ok(Some(testbed))
        } else {
            Err(TestbedError::MultipleTestbeds {
                project: project.to_string(),
                testbed_name: testbed_name.as_ref().into(),
                count: testbed_count,
            })
        }
    } else {
        Ok(None)
    }
}

async fn create_testbed(
    project: &ResourceId,
    testbed_name: ResourceName,
    testbed_slug: Option<Slug>,
    log: bool,
    backend: &AuthBackend,
) -> Result<JsonTestbed, TestbedError> {
    cli_println_quietable!(
        log,
        "Creating a new testbed with name \"{testbed_name}\" in project \"{project}\".",
    );
    let new_testbed = &JsonNewTestbed {
        name: testbed_name.into(),
        slug: testbed_slug.map(Into::into),
        soft: Some(true),
    };

    backend
        .send_with(|client| async move {
            client
                .proj_testbed_post()
                .project(project.clone())
                .body(new_testbed.clone())
                .send()
                .await
        })
        .await
        .map_err(TestbedError::CreateTestbed)
}
