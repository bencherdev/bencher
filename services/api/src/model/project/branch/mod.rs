use bencher_json::{
    project::{
        branch::{JsonNewStartPoint, JsonUpdateBranch},
        report::JsonReportStartPoint,
    },
    BranchName, BranchUuid, DateTime, GitHash, JsonBranch, JsonNewBranch, NameId, NameIdKind, Slug,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, TextExpressionMethods};
use dropshot::HttpError;
use http::StatusCode;
use slog::Logger;
use version::QueryVersion;

use super::{plot::UpdatePlot, ProjectId, QueryProject};
use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{
        assert_parentage, issue_error, resource_conflict_err, resource_not_found_err,
        BencherResource,
    },
    schema::{self, branch as branch_table},
    util::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        name_id::{fn_eq_name_id, fn_from_name_id},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
    },
};

pub mod reference;
pub mod reference_version;
pub mod version;

use reference::{InsertReference, QueryReference, ReferenceId};
use reference_version::{QueryReferenceVersion, ReferenceVersionId, StartPoint};

crate::util::typed_id::typed_id!(BranchId);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = branch_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryBranch {
    pub id: BranchId,
    pub uuid: BranchUuid,
    pub project_id: ProjectId,
    pub name: BranchName,
    pub slug: Slug,
    pub head_id: Option<ReferenceId>,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl QueryBranch {
    fn_eq_resource_id!(branch);
    fn_from_resource_id!(branch, Branch);

    fn_eq_name_id!(BranchName, branch);
    fn_from_name_id!(branch, Branch);

    fn_get!(branch, BranchId);
    fn_get_id!(branch, BranchId, BranchUuid);
    fn_get_uuid!(branch, BranchId, BranchUuid);
    fn_from_uuid!(branch, BranchUuid, Branch);

    pub fn head_id(&self) -> Result<ReferenceId, HttpError> {
        self.head_id.ok_or_else(|| {
            // A branch should always have a head
            let err = issue_error(
                StatusCode::NOT_FOUND,
                "Failed to find branch head",
                &format!("No branch head: {}/{}", self.project_id, self.uuid),
                "branch head reference is null",
            );
            debug_assert!(false, "{err}");
            #[cfg(feature = "sentry")]
            sentry::capture_error(&err);
            err
        })
    }

    pub async fn get_latest_branch_reference_version(
        context: &ApiContext,
        project_id: ProjectId,
        branch: &NameId,
        hash: Option<&GitHash>,
    ) -> Result<BranchReferenceVersion, HttpError> {
        let branch = Self::from_name_id(conn_lock!(context), project_id, branch)?;
        let reference_version =
            QueryReferenceVersion::get_latest_for_branch(context, project_id, &branch, hash)
                .await?;
        Ok(BranchReferenceVersion {
            branch,
            reference_version,
        })
    }

    pub async fn get_or_create(
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        branch: &NameId,
        report_start_point: Option<&JsonReportStartPoint>,
    ) -> Result<(BranchId, ReferenceId), HttpError> {
        let (query_branch, query_reference) =
            Self::get_or_create_inner(log, context, project_id, branch, report_start_point).await?;

        if query_branch.archived.is_some() {
            let update_branch = UpdateBranch::unarchive();
            diesel::update(schema::branch::table.filter(schema::branch::id.eq(query_branch.id)))
                .set(&update_branch)
                .execute(conn_lock!(context))
                .map_err(resource_conflict_err!(Branch, &query_branch))?;
        }

        Ok((query_branch.id, query_reference.id))
    }

    async fn get_or_create_inner(
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        branch: &NameId,
        report_start_point: Option<&JsonReportStartPoint>,
    ) -> Result<(Self, QueryReference), HttpError> {
        let query_branch = Self::from_name_id(conn_lock!(context), project_id, branch);

        let http_error = match query_branch {
            Ok(branch) => {
                return branch
                    .with_start_point(log, context, project_id, report_start_point)
                    .await;
            },
            Err(e) => e,
        };

        let Ok(kind) = NameIdKind::<BranchName>::try_from(branch) else {
            return Err(http_error);
        };
        let branch = match kind {
            NameIdKind::Uuid(_) => return Err(http_error),
            NameIdKind::Slug(slug) => JsonNewBranch {
                name: slug.clone().into(),
                slug: Some(slug),
                start_point: report_start_point.and_then(JsonReportStartPoint::to_new_start_point),
            },
            NameIdKind::Name(name) => JsonNewBranch {
                name,
                slug: None,
                start_point: report_start_point.and_then(JsonReportStartPoint::to_new_start_point),
            },
        };
        InsertBranch::from_json(log, context, project_id, branch).await
    }

    async fn with_start_point(
        self,
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        report_start_point: Option<&JsonReportStartPoint>,
    ) -> Result<(Self, QueryReference), HttpError> {
        // Get the current start point.
        let current_start_point = self.get_start_point(context).await?;
        // Get the new start point, if there is a branch specified.
        let new_start_point = if let Some(JsonReportStartPoint {
            branch: Some(branch),
            hash,
            ..
        }) = report_start_point
        {
            // If updating the start point, it is okay if it does not exist.
            if let Ok(reference_version) =
                QueryReferenceVersion::get_start_point(context, project_id, branch, hash.as_ref())
                    .await
            {
                Some(reference_version.to_start_point(context).await?)
            } else {
                None
            }
        } else {
            None
        };
        let clone_thresholds = report_start_point.and_then(|rsp| rsp.thresholds);

        // If reset is set then the branch needs to be reset.
        if let Some(JsonReportStartPoint {
            reset: Some(true), ..
        }) = report_start_point
        {
            return self
                .rename_and_create(log, context, new_start_point.as_ref(), clone_thresholds)
                .await;
        }

        // Compare the current start point against the new start point.
        match (&current_start_point, &new_start_point) {
            // If there is both a current and new start point, then they need to be compared.
            (Some(current), Some(new)) => {
                // Check if the current and new branches match.
                if current.branch.uuid == new.branch.uuid {
                    // If the current and new start point branches match, then check the hashes.
                    match (&current.version.hash, &new.version.hash) {
                        (Some(current_hash), Some(hash)) => {
                            // If the hashes match, then there is nothing to do.
                            if current_hash == hash {
                                Ok(self)
                            } else {
                                // Rename and create a new branch, if the hashes do not match.
                                self.rename_and_create(
                                    log,
                                    context,
                                    new_start_point.as_ref(),
                                    clone_thresholds,
                                )
                                .await
                            }
                        },
                        // Rename the current branch if it does not have a start point hash and the new start point does.
                        // This should only rarely happen going forward, as most branches with a start point will have a hash.
                        (None, Some(_)) => {
                            self.rename_and_create(
                                log,
                                context,
                                new_start_point.as_ref(),
                                clone_thresholds,
                            )
                            .await
                        },
                        // If a start point hash is not specified, then there is nothing to check.
                        // Even if the current branch has a start point hash, it does not need to always be specified.
                        // That is, setting the start point hash is not required on every run.
                        // Requiring it on every run would be a breaking change
                        // for users who have already specified a start point without a hash.
                        (_, None) => Ok(self),
                    }
                } else {
                    // If the current start point branch does not match the new start point branch,
                    // then the branch needs to be recreated from that new start point.
                    self.rename_and_create(log, context, new_start_point.as_ref(), clone_thresholds)
                        .await
                }
            },
            // If the current branch does not have a start point and one is specified,
            // then the branch needs to be recreated from that start point.
            // The naming convention for this will be a detached branch name and slug is okay: `branch_name@detached`
            (None, Some(_)) => {
                self.rename_and_create(log, context, new_start_point.as_ref(), clone_thresholds)
                    .await
            },
            // If a start point is not specified, then there is nothing to check.
            // Even if the current branch has a start point, it does not need to always be specified.
            // That is, setting the start point is not required on every run.
            (_, None) => Ok(self),
        }
    }

    async fn get_start_point(&self, context: &ApiContext) -> Result<Option<StartPoint>, HttpError> {
        let Some(head_id) = self.head_id else {
            return Ok(None);
        };
        let reference_version = QueryReferenceVersion::get(conn_lock!(context), head_id)?;
        let start_point = reference_version.to_start_point(context).await?;
        Ok(Some(start_point))
    }

    pub async fn rename_and_create(
        self,
        log: &Logger,
        context: &ApiContext,
        new_start_point: Option<&StartPoint>,
        clone_thresholds: Option<bool>,
    ) -> Result<Self, HttpError> {
        // Update the current branch name and slug
        self.rename(context).await?;

        // Create new branch with the same name and slug as the current branch
        let branch = JsonNewBranch {
            name: self.name.clone(),
            slug: Some(self.slug.clone()),
            start_point: new_start_point.map(|start_point| JsonNewStartPoint {
                branch: NameId::from(start_point.branch.uuid),
                hash: start_point.version.hash.clone(),
                thresholds: clone_thresholds,
            }),
        };
        let new_branch = Self::create_from_json(log, context, self.project_id, branch).await?;

        // Update all plots for the current branch and to the new branch
        UpdatePlot::update_branch_for_all_plots(conn_lock!(context), self.id, new_branch.id)?;

        Ok(new_branch)
    }

    pub async fn rename(&self, context: &ApiContext) -> Result<(), HttpError> {
        let branch_name = format!("{name}@{uuid}", name = &self.name, uuid = &self.uuid);
        let count = schema::branch::table
            .filter(schema::branch::name.like(&format!("{branch_name}%")))
            .count()
            .get_result::<i64>(conn_lock!(context))
            .map_err(resource_not_found_err!(Branch, (&self, &branch_name)))?;
        let branch_name = if count > 0 {
            format!("{branch_name}/{count}")
        } else {
            branch_name
        };
        let branch_name = branch_name
            .parse()
            .map_err(resource_conflict_err!(Branch, (&self, &branch_name)))?;

        let branch_slug = conn_lock!(context, |conn| ok_slug!(
            conn,
            self.project_id,
            &branch_name,
            None,
            branch,
            QueryBranch
        )?);

        let json_update_branch = JsonUpdateBranch {
            name: Some(branch_name),
            slug: Some(branch_slug),
            // Auto-archive the current branch
            archived: Some(true),
        };
        let update_branch = UpdateBranch::from(json_update_branch);
        diesel::update(schema::branch::table.filter(schema::branch::id.eq(self.id)))
            .set(&update_branch)
            .execute(conn_lock!(context))
            .map_err(resource_conflict_err!(Branch, (&self, &update_branch)))?;

        Ok(())
    }

    pub fn into_json_for_project(
        self,
        conn: &mut DbConnection,
        project: &QueryProject,
    ) -> Result<JsonBranch, HttpError> {
        let head_id = self.head_id()?;
        let head = QueryReference::get(conn, head_id)?;
        self.into_json_for_head(conn, project, &head, None)
    }

    pub fn into_json_for_head(
        self,
        conn: &mut DbConnection,
        project: &QueryProject,
        head: &QueryReference,
        version: Option<QueryVersion>,
    ) -> Result<JsonBranch, HttpError> {
        let Self {
            uuid,
            project_id,
            name,
            slug,
            created,
            modified,
            archived,
            ..
        } = self;
        assert_parentage(
            BencherResource::Project,
            project.id,
            BencherResource::Branch,
            project_id,
        );
        assert_parentage(
            BencherResource::Branch,
            self.id,
            BencherResource::Reference,
            head.branch_id,
        );
        let head = QueryReference::get_json(conn, head.id, version)?;
        Ok(JsonBranch {
            uuid,
            project: project.uuid,
            name,
            slug,
            head,
            created,
            modified,
            archived,
        })
    }
}

#[derive(Debug, Clone)]
pub struct BranchReferenceVersion {
    pub branch: QueryBranch,
    pub reference_version: QueryReferenceVersion,
}

impl BranchReferenceVersion {
    pub fn reference_version_id(&self) -> ReferenceVersionId {
        self.reference_version.id
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = branch_table)]
pub struct InsertBranch {
    pub uuid: BranchUuid,
    pub project_id: ProjectId,
    pub name: BranchName,
    pub slug: Slug,
    pub head_id: Option<ReferenceId>,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl InsertBranch {
    pub async fn new(
        context: &ApiContext,
        project_id: ProjectId,
        name: BranchName,
        slug: Option<Slug>,
    ) -> Result<Self, HttpError> {
        let slug = conn_lock!(context, |conn| ok_slug!(
            conn,
            project_id,
            &name,
            slug,
            branch,
            QueryBranch
        )?);
        let timestamp = DateTime::now();
        Ok(Self {
            uuid: BranchUuid::new(),
            project_id,
            name,
            slug,
            head_id: None,
            created: timestamp,
            modified: timestamp,
            archived: None,
        })
    }

    pub async fn from_json(
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        branch: JsonNewBranch,
    ) -> Result<(QueryBranch, QueryReference), HttpError> {
        let JsonNewBranch {
            name,
            slug,
            start_point,
        } = branch;

        // Create branch
        let insert_branch = Self::new(context, project_id, name, slug).await?;
        diesel::insert_into(schema::branch::table)
            .values(&insert_branch)
            .execute(conn_lock!(context))
            .map_err(resource_conflict_err!(Branch, insert_branch))?;

        // Get the new branch
        let query_branch = schema::branch::table
            .filter(schema::branch::uuid.eq(&insert_branch.uuid))
            .first::<QueryBranch>(conn_lock!(context))
            .map_err(resource_not_found_err!(Branch, insert_branch))?;

        // Get the branch reference version for the start point
        let branch_start_point = if let Some(start_point) = &start_point {
            Some(
                QueryBranch::get_latest_branch_reference_version(
                    context,
                    project_id,
                    &start_point.branch,
                    start_point.hash.as_ref(),
                )
                .await?,
            )
        } else {
            None
        };

        // Create the head reference for the branch
        let insert_reference = InsertReference::new(
            query_branch.id,
            branch_start_point
                .as_ref()
                .map(BranchReferenceVersion::reference_version_id),
        );

        diesel::insert_into(schema::reference::table)
            .values(&insert_reference)
            .execute(conn_lock!(context))
            .map_err(resource_conflict_err!(Reference, insert_reference))?;

        // Get the new reference
        let query_reference = schema::reference::table
            .filter(schema::reference::uuid.eq(&insert_reference.uuid))
            .first::<QueryReference>(conn_lock!(context))
            .map_err(resource_not_found_err!(Reference, insert_reference))?;

        // Update the branch head reference
        diesel::update(schema::branch::table.filter(schema::branch::id.eq(query_branch.id)))
            .set(schema::branch::head_id.eq(query_reference.id))
            .execute(conn_lock!(context))
            .map_err(resource_conflict_err!(
                Branch,
                (&query_branch, &query_reference)
            ))?;

        // Clone data and optionally thresholds from the start point
        let new_branch_with_thresholds = start_point
            .as_ref()
            .and_then(|sp| sp.thresholds.map(|t| t.then_some(&query_branch)))
            .unwrap_or_default();
        query_reference
            .start_point(
                log,
                context,
                project_id,
                branch_start_point.as_ref(),
                new_branch_with_thresholds,
            )
            .await?;

        Ok((query_branch, query_reference))
    }

    pub async fn main(
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
    ) -> Result<QueryBranch, HttpError> {
        Self::from_json(log, context, project_id, JsonNewBranch::main())
            .await
            .map(|(branch, _)| branch)
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = branch_table)]
pub struct UpdateBranch {
    pub name: Option<BranchName>,
    pub slug: Option<Slug>,
    pub modified: DateTime,
    pub archived: Option<Option<DateTime>>,
}

impl From<JsonUpdateBranch> for UpdateBranch {
    fn from(update: JsonUpdateBranch) -> Self {
        let JsonUpdateBranch {
            name,
            slug,
            archived,
        } = update;
        let modified = DateTime::now();
        let archived = archived.map(|archived| archived.then_some(modified));
        Self {
            name,
            slug,
            modified,
            archived,
        }
    }
}

impl UpdateBranch {
    fn unarchive() -> Self {
        JsonUpdateBranch {
            name: None,
            slug: None,
            archived: Some(false),
        }
        .into()
    }
}
