use bencher_json::{BencherKey, ProjectResourceId};

use crate::{RunError, parser::run::CliRunProject};

pub fn resolve_project(
    run_project: CliRunProject,
    key: Option<&BencherKey>,
    has_image_project: bool,
) -> Result<Option<ProjectResourceId>, RunError> {
    // Project-scoped keys (`bencher_run_*`) are bound to a single existing
    // project at issue time and cannot perform slug auto-creation, so a
    // `--project` is mandatory whenever `--key` is one of them, unless the
    // project can be derived from the job image (`--image`). User-scoped
    // keys (`bencher_user_*`) and JWT tokens have no such restriction.
    if key.is_some_and(BencherKey::is_project)
        && run_project.project.is_none()
        && !has_image_project
    {
        return Err(RunError::ProjectKeyRequiresProject);
    }

    let is_ci = std::env::var("CI").unwrap_or_default() == "true";
    map_project(run_project, has_image_project, is_ci)
}

fn map_project(
    run_project: CliRunProject,
    has_image_project: bool,
    is_ci: bool,
) -> Result<Option<ProjectResourceId>, RunError> {
    if let Some(project_id) = run_project.project {
        Ok(Some(project_id))
    } else if has_image_project {
        // The server derives the project from the job image repository,
        // so there is no risk of on-the-fly project creation in CI
        Ok(None)
    } else if is_ci && !run_project.ci_on_the_fly {
        Err(RunError::CiOnTheFly)
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::{CliRunProject, RunError, map_project};

    fn run_project(project: Option<&str>, ci_on_the_fly: bool) -> CliRunProject {
        CliRunProject {
            project: project.map(|project| project.parse().expect("Invalid project")),
            ci_on_the_fly,
        }
    }

    #[test]
    fn explicit_project() {
        let project = map_project(run_project(Some("my-project"), false), false, true)
            .expect("Explicit project should always be accepted");
        assert!(project.is_some());
    }

    #[test]
    fn no_project_not_ci() {
        let project = map_project(run_project(None, false), false, false)
            .expect("On-the-fly project should be accepted outside of CI");
        assert!(project.is_none());
    }

    #[test]
    fn no_project_ci_errors() {
        let result = map_project(run_project(None, false), false, true);
        assert!(matches!(result, Err(RunError::CiOnTheFly)), "{result:?}");
    }

    #[test]
    fn no_project_ci_on_the_fly() {
        let project = map_project(run_project(None, true), false, true)
            .expect("`--ci-on-the-fly` should allow an on-the-fly project in CI");
        assert!(project.is_none());
    }

    #[test]
    fn image_project_ci() {
        let project = map_project(run_project(None, false), true, true)
            .expect("An image-derived project should be accepted in CI");
        assert!(project.is_none());
    }
}
