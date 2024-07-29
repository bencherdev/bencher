use bencher_json::{
    project::{
        branch::{JsonNewStartPoint, JsonUpdateBranch},
        report::JsonReportStartPoint,
    },
    BranchName, BranchUuid, DateTime, JsonBranch, JsonNewBranch, NameId, NameIdKind, Slug,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, TextExpressionMethods};
use dropshot::HttpError;
use slog::Logger;

use super::{
    branch_version::{BranchVersionId, InsertBranchVersion, QueryBranchVersion, StartPoint},
    plot::UpdatePlot,
    threshold::model::{InsertModel, QueryModel},
    version::{QueryVersion, VersionId},
    ProjectId, QueryProject,
};
use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{assert_parentage, resource_conflict_err, resource_not_found_err, BencherResource},
    model::project::threshold::{InsertThreshold, QueryThreshold},
    schema::{self, branch as branch_table},
    util::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        name_id::{fn_eq_name_id, fn_from_name_id},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
    },
};

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
    pub start_point_id: Option<BranchVersionId>,
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

    pub async fn create_from_json(
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        branch: JsonNewBranch,
    ) -> Result<Self, HttpError> {
        let insert_branch = InsertBranch::from_json(context, project_id, branch.clone()).await?;

        diesel::insert_into(schema::branch::table)
            .values(&insert_branch)
            .execute(conn_lock!(context))
            .map_err(resource_conflict_err!(Branch, insert_branch))?;

        // Clone data and optionally thresholds from the start point
        let clone_thresholds = branch
            .start_point
            .as_ref()
            .and_then(|sp| sp.thresholds)
            .unwrap_or_default();
        insert_branch
            .start_point(log, context, clone_thresholds)
            .await?;

        schema::branch::table
            .filter(schema::branch::uuid.eq(&insert_branch.uuid))
            .first::<Self>(conn_lock!(context))
            .map_err(resource_not_found_err!(Branch, insert_branch))
    }

    pub async fn get_or_create(
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        branch: &NameId,
        report_start_point: Option<&JsonReportStartPoint>,
    ) -> Result<BranchId, HttpError> {
        let query_branch =
            Self::get_or_create_inner(log, context, project_id, branch, report_start_point).await?;

        if query_branch.archived.is_some() {
            let update_branch = UpdateBranch::unarchive();
            diesel::update(schema::branch::table.filter(schema::branch::id.eq(query_branch.id)))
                .set(&update_branch)
                .execute(conn_lock!(context))
                .map_err(resource_conflict_err!(Branch, &query_branch))?;
        }

        Ok(query_branch.id)
    }

    async fn get_or_create_inner(
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        branch: &NameId,
        report_start_point: Option<&JsonReportStartPoint>,
    ) -> Result<Self, HttpError> {
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
        Self::create_from_json(log, context, project_id, branch).await
    }

    async fn with_start_point(
        self,
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        report_start_point: Option<&JsonReportStartPoint>,
    ) -> Result<Self, HttpError> {
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
            if let Ok(branch_version) =
                QueryBranchVersion::get_start_point(context, project_id, branch, hash.as_ref())
                    .await
            {
                Some(branch_version.to_start_point(context).await?)
            } else {
                None
            }
        } else {
            None
        };
        let clone_thresholds = report_start_point.and_then(|rsp| rsp.thresholds);

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
                                    current_start_point.as_ref(),
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
                                current_start_point.as_ref(),
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
                    self.rename_and_create(
                        log,
                        context,
                        current_start_point.as_ref(),
                        new_start_point.as_ref(),
                        clone_thresholds,
                    )
                    .await
                }
            },
            // If the current branch does not have a start point and one is specified,
            // then the branch needs to be recreated from that start point.
            // The naming convention for this will be a detached branch name and slug is okay: `branch_name@detached`
            (None, Some(_)) => {
                self.rename_and_create(
                    log,
                    context,
                    current_start_point.as_ref(),
                    new_start_point.as_ref(),
                    clone_thresholds,
                )
                .await
            },
            // If a start point is not specified, check to see if reset is.
            (_, None) => {
                // If reset is set then the branch needs to be reset.
                if let Some(JsonReportStartPoint {
                    reset: Some(true), ..
                }) = report_start_point
                {
                    self.rename_and_create(
                        log,
                        context,
                        current_start_point.as_ref(),
                        new_start_point.as_ref(),
                        clone_thresholds,
                    )
                    .await
                } else {
                    // If a start point is not specified and reset is not set, then there is nothing to check.
                    // Even if the current branch has a start point, it does not need to always be specified.
                    // That is, setting the start point is not required on every run.
                    Ok(self)
                }
            },
        }
    }

    async fn get_start_point(&self, context: &ApiContext) -> Result<Option<StartPoint>, HttpError> {
        let Some(start_point_id) = self.start_point_id else {
            return Ok(None);
        };
        let branch_version = QueryBranchVersion::get(conn_lock!(context), start_point_id)?;
        let start_point = branch_version.to_start_point(context).await?;
        Ok(Some(start_point))
    }

    pub async fn rename_and_create(
        self,
        log: &Logger,
        context: &ApiContext,
        current_start_point: Option<&StartPoint>,
        new_start_point: Option<&StartPoint>,
        clone_thresholds: Option<bool>,
    ) -> Result<Self, HttpError> {
        // Update the current branch name and slug
        self.rename(context, current_start_point, new_start_point)
            .await?;

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

    pub async fn rename(
        &self,
        context: &ApiContext,
        current_start_point: Option<&StartPoint>,
        new_start_point: Option<&StartPoint>,
    ) -> Result<(), HttpError> {
        let suffix = self.rename_branch_suffix(current_start_point, new_start_point);
        let branch_name = format!("{branch_name}@{suffix}", branch_name = &self.name);

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
        let branch_slug = Slug::new(&branch_name);

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

    fn rename_branch_suffix(
        &self,
        current_start_point: Option<&StartPoint>,
        new_start_point: Option<&StartPoint>,
    ) -> String {
        // If there is no current start point, then the branch will be detached.
        let Some(current_start_point) = current_start_point else {
            return "detached".to_owned();
        };

        // If the start point is self-referential, then simply name it `HEAD` to avoid confusing recursive names.
        // While `HEAD` isn't the most accurate name, it is a reserved name in git so it should not be used by any other branches.
        // Otherwise, just use the name of the current start point branch.
        let branch_name = if new_start_point.is_some_and(|new| self.uuid == new.branch.uuid) {
            "HEAD"
        } else {
            current_start_point.branch.name.as_ref()
        };
        let version_suffix = if let Some(hash) = &current_start_point.version.hash {
            format!("hash/{hash}")
        } else {
            format!("version/{}", current_start_point.version.number)
        };
        format!("{branch_name}/{version_suffix}")
    }

    pub fn into_json_for_project(
        self,
        conn: &mut DbConnection,
        project: &QueryProject,
    ) -> Result<JsonBranch, HttpError> {
        let Self {
            uuid,
            project_id,
            name,
            slug,
            start_point_id,
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
        let start_point = if let Some(start_point_id) = start_point_id {
            Some(QueryBranchVersion::get(conn, start_point_id)?.into_start_point_json(conn)?)
        } else {
            None
        };
        Ok(JsonBranch {
            uuid,
            project: project.uuid,
            name,
            slug,
            start_point,
            created,
            modified,
            archived,
        })
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = branch_table)]
pub struct InsertBranch {
    pub uuid: BranchUuid,
    pub project_id: ProjectId,
    pub name: BranchName,
    pub slug: Slug,
    pub start_point_id: Option<BranchVersionId>,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl InsertBranch {
    pub async fn from_json(
        context: &ApiContext,
        project_id: ProjectId,
        branch: JsonNewBranch,
    ) -> Result<Self, HttpError> {
        let JsonNewBranch {
            name,
            slug,
            start_point,
        } = branch;
        let slug = conn_lock!(context, |conn| ok_slug!(
            conn,
            project_id,
            &name,
            slug,
            branch,
            QueryBranch
        )?);
        let timestamp = DateTime::now();

        let start_point_id = if let Some(JsonNewStartPoint { branch, hash, .. }) = start_point {
            // When creating a new branch, it is okay if the start point does not yet exist.
            // https://github.com/bencherdev/bencher/issues/450
            QueryBranchVersion::get_start_point(context, project_id, &branch, hash.as_ref())
                .await
                .map(|start_point| start_point.id)
                .ok()
        } else {
            None
        };

        Ok(Self {
            uuid: BranchUuid::new(),
            project_id,
            name,
            slug,
            start_point_id,
            created: timestamp,
            modified: timestamp,
            archived: None,
        })
    }

    pub async fn main(context: &ApiContext, project_id: ProjectId) -> Result<Self, HttpError> {
        Self::from_json(context, project_id, JsonNewBranch::main()).await
    }

    pub async fn start_point(
        &self,
        log: &Logger,
        context: &ApiContext,
        clone_thresholds: bool,
    ) -> Result<(), HttpError> {
        let Some(start_point_id) = self.start_point_id else {
            return Ok(());
        };
        let start_point = QueryBranchVersion::get(conn_lock!(context), start_point_id)?;
        let new_branch_id = QueryBranch::get_id(conn_lock!(context), self.uuid)?;

        self.clone_versions(context, &start_point, new_branch_id)
            .await?;

        if clone_thresholds {
            self.clone_thresholds(log, context, &start_point, new_branch_id)
                .await?;
        }

        Ok(())
    }

    async fn clone_versions(
        &self,
        context: &ApiContext,
        start_point: &QueryBranchVersion,
        new_branch_id: BranchId,
    ) -> Result<(), HttpError> {
        let start_point_version = QueryVersion::get(conn_lock!(context), start_point.version_id)?;
        // Get all prior versions (version number less than or equal to) for the start point branch
        let version_ids = schema::branch_version::table
            .inner_join(schema::version::table)
            .filter(schema::branch_version::branch_id.eq(start_point.branch_id))
            .filter(schema::version::number.le(start_point_version.number))
            .order(schema::version::number.desc())
            .select(schema::branch_version::version_id)
            .load::<VersionId>(conn_lock!(context))
            .map_err(resource_not_found_err!(
                BranchVersion,
                (start_point, start_point_version)
            ))?;

        // Add new branch to all start point branch versions
        for version_id in version_ids {
            let insert_branch_version = InsertBranchVersion {
                branch_id: new_branch_id,
                version_id,
            };

            diesel::insert_into(schema::branch_version::table)
                .values(&insert_branch_version)
                .execute(conn_lock!(context))
                .map_err(resource_conflict_err!(BranchVersion, insert_branch_version))?;
        }

        Ok(())
    }

    async fn clone_thresholds(
        &self,
        log: &Logger,
        context: &ApiContext,
        start_point: &QueryBranchVersion,
        new_branch_id: BranchId,
    ) -> Result<(), HttpError> {
        // Get all thresholds for the start point branch
        let query_thresholds = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(start_point.branch_id))
            .load::<QueryThreshold>(conn_lock!(context))
            .map_err(resource_not_found_err!(Threshold, start_point))?;

        // Add new branch to cloned thresholds with cloned current threshold model
        for query_threshold in query_thresholds {
            // Hold the database lock across the entire `clone_threshold` call
            if let Err(e) =
                self.clone_threshold(conn_lock!(context), new_branch_id, &query_threshold)
            {
                slog::warn!(log, "Failed to clone threshold: {e}");
            }
        }

        Ok(())
    }

    fn clone_threshold(
        &self,
        conn: &mut DbConnection,
        new_branch_id: BranchId,
        query_threshold: &QueryThreshold,
    ) -> Result<(), HttpError> {
        // Clone the threshold for the new branch
        let insert_threshold = InsertThreshold::new(
            self.project_id,
            new_branch_id,
            query_threshold.testbed_id,
            query_threshold.measure_id,
        );

        // Create the new threshold
        diesel::insert_into(schema::threshold::table)
            .values(&insert_threshold)
            .execute(conn)
            .map_err(resource_conflict_err!(Threshold, insert_threshold))?;

        // Get the new threshold
        let threshold_id = QueryThreshold::get_id(conn, insert_threshold.uuid)?;

        // Get the current threshold model
        let model_id = query_threshold.model_id()?;
        let query_model = schema::model::table
            .filter(schema::model::id.eq(model_id))
            .first::<QueryModel>(conn)
            .map_err(resource_not_found_err!(Model, query_threshold))?;

        // Clone the current threshold model
        let mut insert_model = InsertModel::from(query_model.clone());
        // Set the new model to the new threshold
        insert_model.threshold_id = threshold_id;
        // Create the new model for the new threshold
        diesel::insert_into(schema::model::table)
            .values(&insert_model)
            .execute(conn)
            .map_err(resource_conflict_err!(Model, insert_model))?;

        // Get the new model
        let model_id = QueryModel::get_id(conn, insert_model.uuid)?;

        // Set the new model for the new threshold
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)))
            .set(schema::threshold::model_id.eq(model_id))
            .execute(conn)
            .map_err(resource_conflict_err!(
                Threshold,
                (&query_threshold, &query_model)
            ))?;

        Ok(())
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
