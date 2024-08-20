use bencher_client::types::JsonNewThreshold;
use bencher_json::{NameId, ResourceId};

use super::model::Model;
use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::threshold::{CliThresholdCreate, CliThresholdCreateProject},
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ResourceId,
    pub branch: NameId,
    pub testbed: NameId,
    pub measure: NameId,
    pub model: Model,
    pub backend: AuthBackend,
}

#[derive(Debug, thiserror::Error)]
pub enum ThresholdError {
    #[error("Failed to find Bencher project. Set the project as the first argument, use the `--project` argument, or the `BENCHER_PROJECT` environment variable.")]
    NoProject,
    #[error("Failed to parse UUID or slug for the project: {0}")]
    ParseProject(bencher_json::ValidError),
}

impl TryFrom<CliThresholdCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliThresholdCreate) -> Result<Self, Self::Error> {
        let CliThresholdCreate {
            project,
            branch,
            testbed,
            measure,
            model,
            backend,
        } = create;
        Ok(Self {
            project: unwrap_project(project)?,
            branch,
            testbed,
            measure,
            model: model.try_into()?,
            backend: backend.try_into()?,
        })
    }
}

fn unwrap_project(project: CliThresholdCreateProject) -> Result<ResourceId, ThresholdError> {
    Ok(if let Some(project) = project.project {
        project
    } else if let Some(project) = project.threshold_project {
        project
    } else {
        return Err(ThresholdError::NoProject);
    })
}

impl From<Create> for JsonNewThreshold {
    fn from(create: Create) -> Self {
        let Create {
            branch,
            testbed,
            model,
            measure,
            ..
        } = create;
        let Model {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        } = model;
        Self {
            branch: branch.into(),
            testbed: testbed.into(),
            measure: measure.into(),
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        }
    }
}

impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_threshold_post()
                    .project(self.project.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
