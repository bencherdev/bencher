use bencher_client::types::JsonNewThreshold;
use bencher_json::{BranchNameId, MeasureNameId, ProjectResourceId, TestbedNameId};

use super::{ThresholdError, model::Model};
use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::threshold::{CliThresholdCreate, CliThresholdCreateProject},
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ProjectResourceId,
    pub branch: BranchNameId,
    pub testbed: TestbedNameId,
    pub measure: MeasureNameId,
    pub model: Model,
    pub backend: AuthBackend,
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

fn unwrap_project(project: CliThresholdCreateProject) -> Result<ProjectResourceId, ThresholdError> {
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
