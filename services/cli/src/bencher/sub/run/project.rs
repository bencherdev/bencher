use bencher_json::ProjectResourceId;

use crate::{RunError, parser::run::CliRunProject};

pub fn map_project(run_project: CliRunProject) -> Result<Option<ProjectResourceId>, RunError> {
    if let Some(project_id) = run_project.project {
        Ok(Some(project_id))
    } else if std::env::var("CI").unwrap_or_default() == "true" && !run_project.ci_on_the_fly {
        Err(RunError::CiOnTheFly)
    } else {
        Ok(None)
    }
}
