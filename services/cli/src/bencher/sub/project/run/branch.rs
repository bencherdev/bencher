use bencher_client::{
    types::{JsonNewBranch, JsonNewStartPoint, JsonUpdateBranch},
    ClientError, ErrorResponse,
};
use bencher_json::{
    project::branch::BRANCH_MAIN_STR, BranchName, GitHash, JsonBranch, JsonBranches,
    JsonStartPoint, NameId, NameIdKind, ResourceId, Slug,
};

use crate::{
    bencher::backend::AuthBackend,
    cli_println_quietable,
    parser::project::run::{CliRunBranch, CliRunHash},
};

use super::BENCHER_BRANCH;

#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone)]
pub struct Branch {
    branch: NameId,
    start_point: Option<StartPoint>,
    reset: bool,
    hash: Option<GitHash>,
}

#[derive(Debug, Clone)]
struct StartPoint {
    branch: NameId,
    hash: Option<GitHash>,
    self_ref: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum BranchError {
    #[error("Failed to parse UUID, slug, or name for the branch: {0}")]
    ParseBranch(bencher_json::ValidError),
    #[error("Failed to get branch with UUID: {0}\nDoes it exist? Branches must already exist when using `--branch` or `BENCHER_BRANCH` with a UUID.\nSee: https://bencher.dev/docs/explanation/branch-selection/")]
    GetBranchUuid(crate::BackendError),
    #[error("Failed to get branch with slug: {0}")]
    GetBranchSlug(crate::BackendError),
    #[error("Failed to query branches: {0}")]
    GetBranches(crate::BackendError),
    #[error(
        "{count} branches were found with name \"{branch_name}\" in project \"{project}\"! Exactly one was expected.\nThis is likely a bug. Please report it here: https://github.com/bencherdev/bencher/issues"
    )]
    MultipleBranches {
        project: String,
        branch_name: String,
        count: usize,
    },
    #[error("Failed to get branch start point: {0}\nDoes it exist? The branch start point must already exist when using `--branch-start-point`\nSee: https://bencher.dev/docs/explanation/branch-selection/")]
    GetStartPoint(crate::BackendError),
    #[error(
        "No branches were found for the start point with name \"{branch_name}\" in project \"{project}\". Exactly one was expected.\nDoes it exist?"
    )]
    NoStartPoint {
        project: String,
        branch_name: String,
    },
    #[error("Failed to get current branch start point. This is likely a bug. Please report it here: https://github.com/bencherdev/bencher/issues")]
    GetCurrentStartPoint(crate::BackendError),
    #[error("Failed to create new branch: {0}")]
    CreateBranch(crate::BackendError),
    #[error("Failed to update detached branch: {0}")]
    UpdateBranch(crate::BackendError),
    #[error("Failed to update branch to reserve next hash: {0}")]
    NextBranchHash(crate::BackendError),
}

impl TryFrom<CliRunBranch> for Branch {
    type Error = BranchError;

    fn try_from(run_branch: CliRunBranch) -> Result<Self, Self::Error> {
        let CliRunBranch {
            branch,
            branch_start_point,
            branch_start_point_hash,
            branch_reset,
            deprecated: _,
            hash,
        } = run_branch;
        let branch = if let Some(branch) = branch {
            branch
        } else if let Ok(env_branch) = std::env::var(BENCHER_BRANCH) {
            env_branch
                .as_str()
                .parse()
                .map_err(BranchError::ParseBranch)?
        } else if let Some(branch) = find_branch() {
            branch
        } else {
            BRANCH_MAIN_STR.parse().map_err(BranchError::ParseBranch)?
        };
        let start_point = branch_start_point.first().cloned().and_then(|b| {
            // The only invalid `NameId` is an empty string.
            // This allows for "continue on empty" semantics for the branch start point.
            let start_point_branch = b.parse().ok()?;
            let self_ref = branch == start_point_branch;
            Some(StartPoint {
                branch: start_point_branch,
                hash: branch_start_point_hash,
                self_ref,
            })
        });
        Ok(Self {
            branch,
            start_point,
            reset: branch_reset,
            hash: map_hash(hash),
        })
    }
}

fn find_branch() -> Option<NameId> {
    if let Some(repo) = find_repo() {
        if let Ok(Some(branch)) = repo.head_name() {
            return branch.shorten().to_string().parse().ok();
        }
    }
    None
}

fn map_hash(CliRunHash { hash, no_hash }: CliRunHash) -> Option<GitHash> {
    if let Some(hash) = hash {
        return Some(hash);
    } else if no_hash {
        return None;
    }
    let repo = find_repo()?;
    let head_id = repo.head_id().ok()?;
    let head_object = head_id.object().ok()?;
    Some(head_object.id.into())
}

fn find_repo() -> Option<gix::Repository> {
    let current_dir = std::env::current_dir().ok()?;
    for directory in current_dir.ancestors() {
        if let Ok(repo) = gix::open(directory) {
            return Some(repo);
        }
    }
    None
}

impl Branch {
    pub async fn get(
        &self,
        project: &ResourceId,
        dry_run: bool,
        log: bool,
        backend: &AuthBackend,
    ) -> Result<(NameId, Option<GitHash>), BranchError> {
        if !dry_run {
            self.exists_or_create(project, log, backend).await?;
        }
        Ok((self.branch.clone(), self.hash.clone()))
    }

    async fn exists_or_create(
        &self,
        project: &ResourceId,
        log: bool,
        backend: &AuthBackend,
    ) -> Result<(), BranchError> {
        // Check to make sure that the branch exists before running the benchmarks.
        // Then check that the start point branch matches, if specified.
        let branch = match (&self.branch)
            .try_into()
            .map_err(BranchError::ParseBranch)?
        {
            NameIdKind::Uuid(uuid) => {
                let branch = get_branch(project, &uuid.into(), backend)
                    .await
                    .map_err(BranchError::GetBranchUuid)?;
                self.check_start_point(project, &branch, log, backend)
                    .await?;
                branch
            },
            NameIdKind::Slug(slug) => {
                match get_branch(project, &slug.clone().into(), backend).await {
                    Ok(branch) => {
                        self.check_start_point(project, &branch, log, backend)
                            .await?;
                        branch
                    },
                    Err(crate::BackendError::Client(ClientError::ErrorResponse(
                        ErrorResponse {
                            status: reqwest::StatusCode::NOT_FOUND,
                            ..
                        },
                    ))) => {
                        cli_println_quietable!(
                            log,
                            "Failed to find branch with slug \"{slug}\" in project \"{project}\"."
                        );
                        create_branch(
                            project,
                            slug.clone().into(),
                            Some(slug),
                            self.start_point.clone(),
                            log,
                            backend,
                        )
                        .await?
                    },
                    Err(e) => return Err(BranchError::GetBranchSlug(e)),
                }
            },
            NameIdKind::Name(name) => match get_branch_by_name(project, &name, backend).await {
                Ok(Some(branch)) => {
                    self.check_start_point(project, &branch, log, backend)
                        .await?;
                    branch
                },
                Ok(None) => {
                    cli_println_quietable!(
                        log,
                        "Failed to find branch with name \"{name}\" in project \"{project}\"."
                    );
                    create_branch(project, name, None, self.start_point.clone(), log, backend)
                        .await?
                },
                Err(e) => return Err(e),
            },
        };

        // If there is a hash specified for this report,
        // then go ahead and reserve it as the next hash for the branch.
        // This will help prevent race conditions for any other branches that use this one as their start point.
        reserve_hash(project, &branch, self.hash.as_ref(), backend).await
    }

    async fn check_start_point(
        &self,
        project: &ResourceId,
        current_branch: &JsonBranch,
        log: bool,
        backend: &AuthBackend,
    ) -> Result<(), BranchError> {
        // Compare the current start point against the provided start point.
        match (current_branch.start_point.clone(), self.start_point.clone()) {
            // If there is both a current and provided start point, then they need to be compared.
            (Some(current_start_point), Some(start_point)) => {
                // Get the branch for the provided start point.
                let start_point_branch = match (&start_point.branch)
                    .try_into()
                    .map_err(BranchError::ParseBranch)?
                {
                    NameIdKind::Uuid(uuid) => get_branch(project, &uuid.into(), backend)
                        .await
                        .map_err(BranchError::GetStartPoint),
                    NameIdKind::Slug(slug) => get_branch(project, &slug.into(), backend)
                        .await
                        .map_err(BranchError::GetStartPoint),
                    NameIdKind::Name(name) => get_branch_by_name(project, &name, backend)
                        .await?
                        .ok_or_else(|| BranchError::NoStartPoint {
                            project: project.to_string(),
                            branch_name: name.as_ref().into(),
                        }),
                }?;

                // If the current start point branch does not match the provided start point branch, then the branch needs to be recreated from that new start point.
                if current_start_point.branch != start_point_branch.uuid {
                    cli_println_quietable!(
                        log,
                        "Current start point branch ({current}) is different than the specified start point branch ({specified}).",
                        current = current_start_point.branch,
                        specified = start_point_branch.uuid,
                    );
                    return rename_and_create_branch(
                        project,
                        current_branch,
                        Some(start_point),
                        log,
                        backend,
                    )
                    .await;
                }

                // If both the current and provided start point branches match, then check the hashes.
                match (&current_start_point.version.hash, &start_point.hash) {
                    (Some(current_hash), Some(hash)) => {
                        // Rename and create a new branch if the hashes do not match.
                        if current_hash != hash {
                            cli_println_quietable!(
                                log,
                                "Current start point branch hash ({current_hash}) is different than the specified start point branch hash ({hash}).",
                            );
                            rename_and_create_branch(
                                project,
                                current_branch,
                                Some(start_point),
                                log,
                                backend,
                            )
                            .await?;
                        }
                    },
                    // Rename the current branch if it does not have a start point hash and the provided start point does.
                    // This should only rarely happen going forward, as most branches with a start point will have a hash.
                    (None, Some(_)) => {
                        cli_println_quietable!(
                            log,
                            "No current start point branch hash and a start point branch hash was specified.",
                        );
                        rename_and_create_branch(
                            project,
                            current_branch,
                            Some(start_point),
                            log,
                            backend,
                        )
                        .await?;
                    },
                    // If a start point hash is not specified, then there is nothing to check.
                    // Even if the current branch has a start point hash, it does not need to always be specified.
                    // That is, adding a start point hash is a one way operation with `bencher run`.
                    // Alternatively, this could actually follow the HEAD here, so not specifying a hash is equivalent to specifying the HEAD.
                    // However, that behavior will likely be confusing to users.
                    // Further, this would be a breaking change for users who have already specified a start point without a hash.
                    (_, None) => {},
                }
            },
            // If the current branch does not have a start point and one is specified, then the branch needs to be recreated from that start point.
            // Because adding a start point is a one way operation with `bencher run`, this operation will only ever be performed once.
            // Therefore, using a set naming convention for the detached branch name and slug is okay: `branch_name@detached`
            (None, Some(start_point)) => {
                cli_println_quietable!(
                    log,
                    "No current start point branch and a start point branch was specified.",
                );
                rename_and_create_branch(project, current_branch, Some(start_point), log, backend)
                    .await?;
            },
            // If a start point is not specified and reset is not set, then there is nothing to check.
            // Even if the current branch has a start point, it does not need to always be specified.
            // That is, adding a start point is a one way operation with `bencher run`.
            // Alternatively, this could actually rename and create a new branch if there is a current start point,
            // so not specifying a start point when there is a current start point is equivalent to resetting the branch.
            // However, that behavior will likely be confusing to users.
            (_, None) => {
                // If reset is set then the branch needs to be reset.
                if self.reset {
                    cli_println_quietable!(log, "Branch will be reset to an empty start point.",);
                    rename_and_create_branch(project, current_branch, None, log, backend).await?;
                }
            },
        }

        Ok(())
    }
}

impl StartPoint {
    // Check to see if the start point branch matches the specified branch.
    // If so, then the branch will start from the renamed version of itself.
    fn rename_self_ref(mut self, branch: &JsonBranch) -> Self {
        if self.self_ref {
            self.branch = branch.uuid.into();
            self
        } else {
            self
        }
    }
}

async fn get_branch(
    project: &ResourceId,
    branch: &ResourceId,
    backend: &AuthBackend,
) -> Result<JsonBranch, crate::BackendError> {
    backend
        .send_with(|client| async move {
            client
                .proj_branch_get()
                .project(project.clone())
                .branch(branch.clone())
                .send()
                .await
        })
        .await
}

async fn get_branch_by_name(
    project: &ResourceId,
    branch_name: &BranchName,
    backend: &AuthBackend,
) -> Result<Option<JsonBranch>, BranchError> {
    let json_branches: JsonBranches = backend
        .send_with(|client| async move {
            client
                .proj_branches_get()
                .project(project.clone())
                .name(branch_name.clone())
                .send()
                .await
        })
        .await
        .map_err(BranchError::GetBranches)?;

    let mut json_branches = json_branches.into_inner();
    let branch_count = json_branches.len();
    if let Some(branch) = json_branches.pop() {
        if branch_count == 1 {
            Ok(Some(branch))
        } else {
            Err(BranchError::MultipleBranches {
                project: project.to_string(),
                branch_name: branch_name.as_ref().into(),
                count: branch_count,
            })
        }
    } else {
        Ok(None)
    }
}

async fn create_branch(
    project: &ResourceId,
    branch_name: BranchName,
    branch_slug: Option<Slug>,
    start_point: Option<StartPoint>,
    log: bool,
    backend: &AuthBackend,
) -> Result<JsonBranch, BranchError> {
    let (start_point, message) = if let Some(StartPoint { branch, hash, .. }) = start_point {
        // Default to cloning the thresholds from the start point branch
        let start_point = JsonNewStartPoint {
            branch: branch.clone().into(),
            hash: hash.clone().map(Into::into),
            thresholds: Some(true),
        };
        let message = format!(
            " with start point branch \"{branch}\"{}",
            hash.as_ref()
                .map(|hash| format!(" and hash \"{hash}\""))
                .unwrap_or_default(),
        );
        (Some(start_point), Some(message))
    } else {
        (None, None)
    };
    cli_println_quietable!(
        log,
        "Creating a new branch with name \"{branch_name}\" in project \"{project}\"{message}.",
        message = message.unwrap_or_default()
    );
    let new_branch = &JsonNewBranch {
        name: branch_name.into(),
        slug: branch_slug.map(Into::into),
        soft: Some(true),
        start_point,
    };

    backend
        .send_with(|client| async move {
            client
                .proj_branch_post()
                .project(project.clone())
                .body(new_branch.clone())
                .send()
                .await
        })
        .await
        .map_err(BranchError::CreateBranch)
}

async fn rename_and_create_branch(
    project: &ResourceId,
    current_branch: &JsonBranch,
    start_point: Option<StartPoint>,
    log: bool,
    backend: &AuthBackend,
) -> Result<(), BranchError> {
    // Update the current branch name and slug
    let renamed_branch =
        rename_branch(project, current_branch, start_point.as_ref(), log, backend).await?;
    // Update the start point branch if using a self-referential start point
    let start_point = start_point.map(|sp| sp.rename_self_ref(&renamed_branch));
    // Create new branch with the same name and slug as the current branch
    create_branch(
        project,
        current_branch.name.clone(),
        Some(current_branch.slug.clone()),
        start_point,
        log,
        backend,
    )
    .await?;
    Ok(())
}

async fn rename_branch(
    project: &ResourceId,
    current_branch: &JsonBranch,
    start_point: Option<&StartPoint>,
    log: bool,
    backend: &AuthBackend,
) -> Result<JsonBranch, BranchError> {
    cli_println_quietable!(
        log,
        "New start point for branch with name \"{branch_name}\" in project \"{project}\".",
        branch_name = current_branch.name.as_ref(),
    );

    let suffix = rename_branch_suffix(
        project,
        current_branch.start_point.as_ref(),
        start_point,
        backend,
    )
    .await?;
    let branch_name = format!(
        "{branch_name}@{suffix}",
        branch_name = current_branch.name.as_ref()
    );
    let branch_slug = Slug::new(&branch_name);
    cli_println_quietable!(
        log,
        "Renaming detached branch to have name \"{branch_name}\" and slug \"{branch_slug}\" in project \"{project}\"."
    );

    // TODO archive the detached branch
    match update_branch(
        project,
        &current_branch.uuid.into(),
        Some(branch_name.clone().into()),
        Some(branch_slug.clone().into()),
        backend,
    )
    .await
    {
        Ok(branch) => Ok(branch),
        Err(BranchError::UpdateBranch(crate::BackendError::Client(
            ClientError::ErrorResponse(ErrorResponse {
                status: reqwest::StatusCode::CONFLICT,
                ..
            }),
        ))) => {
            cli_println_quietable!(
                log,
                "Branch with name \"{branch_name}\" or slug \"{branch_slug}\" in project \"{project}\" already exists."
            );
            let branch_name = format!("{branch_name}/{random}", random = Slug::rand_suffix());
            let branch_slug = Slug::new(&branch_name);
            cli_println_quietable!(
                log,
                "Renaming detached branch to have name \"{branch_name}\" and slug \"{branch_slug}\" in project \"{project}\" to avoid conflict."
            );
            update_branch(
                project,
                &current_branch.uuid.into(),
                Some(branch_name.clone().into()),
                Some(branch_slug.clone().into()),
                backend,
            )
            .await
        },
        Err(e) => Err(e),
    }
}

async fn update_branch(
    project: &ResourceId,
    resource_id: &ResourceId,
    name: Option<bencher_client::types::BranchName>,
    slug: Option<bencher_client::types::Slug>,
    backend: &AuthBackend,
) -> Result<JsonBranch, BranchError> {
    let update_branch = &JsonUpdateBranch {
        name,
        slug,
        hash: None,
    };
    backend
        .send_with(|client| async move {
            client
                .proj_branch_patch()
                .project(project.clone())
                .branch(resource_id.clone())
                .body(update_branch.clone())
                .send()
                .await
        })
        .await
        .map_err(BranchError::UpdateBranch)
}

async fn rename_branch_suffix(
    project: &ResourceId,
    current_start_point: Option<&JsonStartPoint>,
    start_point: Option<&StartPoint>,
    backend: &AuthBackend,
) -> Result<String, BranchError> {
    // If there is no current start point, then the branch will be detached.
    let Some(current_start_point) = current_start_point else {
        return Ok("detached".to_owned());
    };

    // Get the current start point branch, with its UUID.
    let current_start_point_branch =
        get_branch(project, &current_start_point.branch.into(), backend)
            .await
            .map_err(BranchError::GetCurrentStartPoint)?;

    // If the start point is self-referential, then simply name it `HEAD` to avoid confusing recursive names.
    // While `HEAD` isn't the most accurate name, it is a reserved name in git so it should not be used by any other branches.
    // Otherwise, just use the name of the current start point branch.
    let branch_name = if start_point.map(|sp| sp.self_ref).unwrap_or_default() {
        "HEAD"
    } else {
        current_start_point_branch.name.as_ref()
    };
    let version_suffix = if let Some(hash) = &current_start_point.version.hash {
        format!("hash/{hash}")
    } else {
        format!("version/{}", current_start_point.version.number)
    };
    Ok(format!("{branch_name}/{version_suffix}"))
}

async fn reserve_hash(
    project: &ResourceId,
    branch: &JsonBranch,
    hash: Option<&GitHash>,
    backend: &AuthBackend,
) -> Result<(), BranchError> {
    // If there is no hash specified, then there is nothing to reserve.
    let Some(hash) = hash else {
        return Ok(());
    };

    let slug = &ResourceId::from(branch.slug.clone());
    let update_branch = &JsonUpdateBranch {
        name: None,
        slug: None,
        hash: Some(hash.clone().into()),
    };
    backend
        .send(|client| async move {
            client
                .proj_branch_patch()
                .project(project.clone())
                .branch(slug.clone())
                .body(update_branch.clone())
                .send()
                .await
        })
        .await
        .map_err(BranchError::NextBranchHash)?;

    Ok(())
}
