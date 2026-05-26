use bencher_json::ProjectResourceId;

use crate::{RunError, parser::run::CliRunProject};

/// Resolve the project for the run.
///
/// Precedence:
/// 1. An explicit `--project` (or `BENCHER_PROJECT`) always wins.
/// 2. For bare-metal runs (`--image`), the project can be derived from the
///    image's repository (e.g. `registry.bencher.dev/<project>:<tag>`); the
///    caller passes the derived id as `image_project` and we adopt it.
/// 3. Outside CI, returning `None` is fine — the API will create the project
///    on-the-fly from the run context.
/// 4. Inside CI without `--ci-on-the-fly`, refuse to silently create.
pub fn map_project(
    run_project: CliRunProject,
    image_project: Option<ProjectResourceId>,
) -> Result<Option<ProjectResourceId>, RunError> {
    if let Some(project_id) = run_project.project {
        return Ok(Some(project_id));
    }
    if let Some(image_project) = image_project {
        return Ok(Some(image_project));
    }
    if std::env::var("CI").unwrap_or_default() == "true" && !run_project.ci_on_the_fly {
        return Err(RunError::CiOnTheFly);
    }
    Ok(None)
}
