use bencher_json::{
    project::{branch::JsonUpdateBranch, report::JsonReportStartPoint},
    BranchName, BranchUuid, DateTime, JsonBranch, JsonNewBranch, NameId, NameIdKind, Slug,
};
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl, SelectableHelper};
use dropshot::HttpError;
use http::StatusCode;
use slog::Logger;
use version::{QueryVersion, VersionId};

use super::{ProjectId, QueryProject};
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
pub mod start_point;
pub mod version;

use reference::{InsertReference, QueryReference, ReferenceId};
use reference_version::QueryReferenceVersion;
use start_point::StartPoint;

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

    pub fn head(&self, conn: &mut DbConnection) -> Result<QueryReference, HttpError> {
        QueryReference::get(conn, self.head_id()?)
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
        // Get the current start point, if one exists.
        let current_start_point = self.get_start_point(context).await?;
        // Get the new start point, if there is a branch specified.
        let new_start_point =
            StartPoint::from_report_json(context, project_id, report_start_point).await?;

        // If reset is set then the branch needs to be reset.
        if let Some(JsonReportStartPoint {
            reset: Some(true), ..
        }) = report_start_point
        {
            return InsertReference::for_branch(log, context, self, new_start_point.as_ref()).await;
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
                                self.into_branch_and_head(context).await
                            } else {
                                // Rename and create a new branch, if the hashes do not match.
                                InsertReference::for_branch(
                                    log,
                                    context,
                                    self,
                                    new_start_point.as_ref(),
                                )
                                .await
                            }
                        },
                        // Rename the current branch if it does not have a start point hash and the new start point does.
                        // This should only rarely happen going forward, as most branches with a start point will have a hash.
                        (None, Some(_)) => {
                            InsertReference::for_branch(
                                log,
                                context,
                                self,
                                new_start_point.as_ref(),
                            )
                            .await
                        },
                        // If a start point hash is not specified, then there is nothing to check.
                        // Even if the current branch has a start point hash, it does not need to always be specified.
                        // That is, setting the start point hash is not required on every run.
                        // Requiring it on every run would be a breaking change
                        // for users who have already specified a start point without a hash.
                        (_, None) => self.into_branch_and_head(context).await,
                    }
                } else {
                    // If the current start point branch does not match the new start point branch,
                    // then the branch needs to be recreated from that new start point.
                    InsertReference::for_branch(log, context, self, new_start_point.as_ref()).await
                }
            },
            // If the current branch does not have a start point and one is specified,
            // then the branch needs to be recreated from that start point.
            (None, Some(_)) => {
                InsertReference::for_branch(log, context, self, new_start_point.as_ref()).await
            },
            // If a start point is not specified, then there is nothing to check.
            // Even if the current branch has a start point, it does not need to always be specified.
            // That is, setting the start point is not required on every run.
            (_, None) => self.into_branch_and_head(context).await,
        }
    }

    async fn get_start_point(&self, context: &ApiContext) -> Result<Option<StartPoint>, HttpError> {
        // Get the head reference for the branch.
        let head = self.head(conn_lock!(context))?;
        // Check to see if the head reference has a start point.
        let Some(start_point_id) = head.start_point_id else {
            return Ok(None);
        };
        // If the head reference has a start point, then get the reference version for the start point.
        let start_point_reference_version =
            QueryReferenceVersion::get(conn_lock!(context), start_point_id)?;
        // Get the branch for the start point reference version.
        let start_point_branch = schema::branch::table
            .inner_join(
                schema::reference::table.on(schema::reference::branch_id.eq(schema::branch::id)),
            )
            .filter(schema::reference::id.eq(start_point_reference_version.reference_id))
            .select(Self::as_select())
            .first::<Self>(conn_lock!(context))
            .map_err(resource_not_found_err!(
                ReferenceVersion,
                &start_point_reference_version
            ))?;
        // Create the branch start point for the branch and reference version.
        StartPoint::new(
            context,
            start_point_branch,
            start_point_reference_version,
            None,
        )
        .await
        .map(Some)
    }

    pub async fn into_branch_and_head(
        self,
        context: &ApiContext,
    ) -> Result<(Self, QueryReference), HttpError> {
        let head = self.head(conn_lock!(context))?;
        Ok((self, head))
    }

    pub fn into_json_for_project(
        self,
        conn: &mut DbConnection,
        project: &QueryProject,
    ) -> Result<JsonBranch, HttpError> {
        let head = self.head(conn)?;
        self.into_json_for_head(conn, project, &head, None)
    }

    pub async fn get_json_for_report(
        context: &ApiContext,
        project: &QueryProject,
        reference_id: ReferenceId,
        version_id: VersionId,
    ) -> Result<JsonBranch, HttpError> {
        let reference = QueryReference::get(conn_lock!(context), reference_id)?;
        let version = QueryVersion::get(conn_lock!(context), version_id)?;
        let branch = QueryBranch::get(conn_lock!(context), reference.branch_id)?;
        branch.into_json_for_head(conn_lock!(context), project, &reference, Some(version))
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
        slog::debug!(log, "Created branch {insert_branch:?}");

        // Get the new branch
        let query_branch = schema::branch::table
            .filter(schema::branch::uuid.eq(&insert_branch.uuid))
            .first::<QueryBranch>(conn_lock!(context))
            .map_err(resource_not_found_err!(Branch, insert_branch))?;
        slog::debug!(log, "Got branch {query_branch:?}");

        // Get the branch reference version for the start point
        let branch_start_point = if let Some(start_point) = start_point {
            // It is okay if the start point does not exist.
            // This prevents a race condition when creating both the branch and start point in CI.
            StartPoint::from_json(context, project_id, start_point)
                .await
                .ok()
        } else {
            None
        };
        slog::debug!(log, "Using start point {branch_start_point:?}");

        InsertReference::for_branch(log, context, query_branch, branch_start_point.as_ref()).await
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
