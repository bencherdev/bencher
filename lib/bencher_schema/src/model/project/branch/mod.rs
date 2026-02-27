use bencher_json::{
    BranchName, BranchNameId, BranchSlug, BranchUuid, DateTime, JsonBranch, JsonNewBranch, NameId,
    project::branch::{JsonUpdateBranch, JsonUpdateStartPoint},
};
use diesel::{
    Connection as _, ExpressionMethods as _, JoinOnDsl as _, QueryDsl as _, RunQueryDsl as _,
    SelectableHelper as _,
};
use dropshot::HttpError;
use slog::Logger;
use version::{QueryVersion, VersionId};

use super::{ProjectId, QueryProject};
use crate::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::{
        BencherResource, assert_parentage, issue_error, resource_conflict_err,
        resource_not_found_err,
    },
    macros::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        name_id::{fn_eq_name_id, fn_from_name_id},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
        sql::last_insert_rowid,
    },
    schema::{self, branch as branch_table},
    write_conn,
};

pub mod head;
pub mod head_version;
pub mod start_point;
pub mod version;

use head::{HeadId, InsertHead, QueryHead};
use head_version::QueryHeadVersion;
use start_point::StartPoint;

crate::macros::typed_id::typed_id!(BranchId);

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
    pub slug: BranchSlug,
    pub head_id: Option<HeadId>,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl QueryBranch {
    fn_eq_resource_id!(branch, BranchResourceId);
    fn_from_resource_id!(project_id, ProjectId, branch, Branch, BranchResourceId);

    fn_eq_name_id!(BranchName, branch, BranchNameId);
    fn_from_name_id!(branch, Branch, BranchNameId);

    fn_get!(branch, BranchId);
    fn_get_id!(branch, BranchId, BranchUuid);
    fn_get_uuid!(branch, BranchId, BranchUuid);
    fn_from_uuid!(project_id, ProjectId, branch, BranchUuid, Branch);

    pub fn head_id(&self) -> Result<HeadId, HttpError> {
        self.head_id.ok_or_else(|| {
            // A branch should always have a head
            let err = issue_error(
                "Failed to find branch head",
                &format!("No branch head: {}/{}", self.project_id, self.uuid),
                "branch head is null",
            );
            debug_assert!(false, "{err}");
            #[cfg(feature = "sentry")]
            sentry::capture_error(&err);
            err
        })
    }

    pub fn head(&self, conn: &mut DbConnection) -> Result<QueryHead, HttpError> {
        QueryHead::get(conn, self.head_id()?)
    }

    pub async fn get_or_create(
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        branch: &BranchNameId,
        start_point: Option<&JsonUpdateStartPoint>,
    ) -> Result<(BranchId, HeadId), HttpError> {
        let (query_branch, query_head) =
            Self::get_or_create_inner(log, context, project_id, branch, start_point).await?;

        if query_branch.archived.is_some() {
            let update_branch = UpdateBranch::unarchive();
            diesel::update(schema::branch::table.filter(schema::branch::id.eq(query_branch.id)))
                .set(&update_branch)
                .execute(write_conn!(context))
                .map_err(resource_conflict_err!(Branch, &query_branch))?;
        }

        Ok((query_branch.id, query_head.id))
    }

    async fn get_or_create_inner(
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        branch: &BranchNameId,
        start_point: Option<&JsonUpdateStartPoint>,
    ) -> Result<(Self, QueryHead), HttpError> {
        let query_branch = Self::from_name_id(auth_conn!(context), project_id, branch);

        let http_error = match query_branch {
            Ok(branch) => {
                return branch
                    .update_start_point_if_changed(log, context, project_id, start_point)
                    .await;
            },
            Err(e) => e,
        };

        let branch = match branch.clone() {
            NameId::Uuid(_) => return Err(http_error),
            NameId::Slug(slug) => JsonNewBranch {
                name: slug.clone().into(),
                slug: Some(slug),
                start_point: start_point.cloned().and_then(Into::into),
            },
            NameId::Name(name) => JsonNewBranch {
                name,
                slug: None,
                start_point: start_point.cloned().and_then(Into::into),
            },
        };
        Self::create_with_head(log, context, project_id, branch).await
    }

    pub async fn create(
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        json_branch: JsonNewBranch,
    ) -> Result<Self, HttpError> {
        Ok(
            Self::create_with_head(log, context, project_id, json_branch)
                .await?
                .0,
        )
    }

    async fn create_with_head(
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        json_branch: JsonNewBranch,
    ) -> Result<(Self, QueryHead), HttpError> {
        #[cfg(feature = "plus")]
        InsertBranch::rate_limit(context, project_id).await?;
        InsertBranch::from_json(log, context, project_id, json_branch).await
    }

    pub async fn update_start_point_if_changed(
        self,
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        start_point: Option<&JsonUpdateStartPoint>,
    ) -> Result<(Self, QueryHead), HttpError> {
        // Get the current start point, if one exists.
        let current_start_point = self.get_start_point(context).await?;
        // Get the new start point, if there is one specified.
        let new_start_point =
            StartPoint::from_update_json(context, project_id, start_point).await?;

        // If reset is set then the branch head needs to be reset.
        if let Some(JsonUpdateStartPoint {
            reset: Some(true), ..
        }) = start_point
        {
            return InsertHead::for_branch(log, context, self, new_start_point.as_ref()).await;
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
                                // If the hashes do not match, create a new branch head.
                                InsertHead::for_branch(log, context, self, new_start_point.as_ref())
                                    .await
                            }
                        },
                        // If there is no current start point hash and the new start point has a start point hash,
                        // then the branch head needs to be recreated from the new start point.
                        // This should only rarely happen going forward, as most branches with a start point will have a hash.
                        (None, Some(_)) => {
                            InsertHead::for_branch(log, context, self, new_start_point.as_ref())
                                .await
                        },
                        // If a start point hash is not specified, then there is nothing to check.
                        // Even if the current branch head has a start point hash, it does not need to always be specified.
                        // That is, setting the start point hash is not required on every run.
                        // Requiring it on every run would be a breaking change
                        // for users who have already specified a start point without a hash.
                        (_, None) => self.into_branch_and_head(context).await,
                    }
                } else {
                    // If the current start point branch does not match the new start point branch,
                    // then the branch head needs to be recreated from the new start point.
                    InsertHead::for_branch(log, context, self, new_start_point.as_ref()).await
                }
            },
            // If the current branch does not have a start point and one is specified,
            // then the branch head needs to be recreated from the new start point.
            (None, Some(_)) => {
                InsertHead::for_branch(log, context, self, new_start_point.as_ref()).await
            },
            // If a start point is not specified, then there is nothing to check.
            // Even if the current branch has a start point, it does not need to always be specified.
            // That is, setting the start point is not required on every run.
            (_, None) => self.into_branch_and_head(context).await,
        }
    }

    async fn get_start_point(&self, context: &ApiContext) -> Result<Option<StartPoint>, HttpError> {
        // Get the head for the branch.
        let head = self.head(auth_conn!(context))?;
        // Check to see if the head has a start point.
        let Some(start_point_id) = head.start_point_id else {
            return Ok(None);
        };
        // If the head has a start point, then get the head version for the start point.
        let start_point_head_version = QueryHeadVersion::get(auth_conn!(context), start_point_id)?;
        // Get the branch for the start point head version.
        let start_point_branch = schema::branch::table
            .inner_join(schema::head::table.on(schema::head::branch_id.eq(schema::branch::id)))
            .filter(schema::head::id.eq(start_point_head_version.head_id))
            .select(Self::as_select())
            .first::<Self>(auth_conn!(context))
            .map_err(resource_not_found_err!(
                HeadVersion,
                &start_point_head_version
            ))?;
        // Create the branch start point for the branch and head version.
        StartPoint::new(
            context,
            start_point_branch,
            start_point_head_version,
            None,
            None,
        )
        .await
        .map(Some)
    }

    pub async fn into_branch_and_head(
        self,
        context: &ApiContext,
    ) -> Result<(Self, QueryHead), HttpError> {
        let head = self.head(auth_conn!(context))?;
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

    pub fn get_json_for_report(
        conn: &mut DbConnection,
        project: &QueryProject,
        head_id: HeadId,
        version_id: VersionId,
    ) -> Result<JsonBranch, HttpError> {
        let head = QueryHead::get(conn, head_id)?;
        let version = QueryVersion::get(conn, version_id)?;
        let branch = QueryBranch::get(conn, head.branch_id)?;
        branch.into_json_for_head(conn, project, &head, Some(version))
    }

    pub fn into_json_for_head(
        self,
        conn: &mut DbConnection,
        project: &QueryProject,
        head: &QueryHead,
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
            BencherResource::Head,
            head.branch_id,
        );
        let head = QueryHead::get_head_json(conn, head.id, version)?;
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
    pub slug: BranchSlug,
    pub head_id: Option<HeadId>,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl InsertBranch {
    #[cfg(feature = "plus")]
    crate::macros::rate_limit::fn_rate_limit!(branch, Branch);

    async fn from_json(
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        branch: JsonNewBranch,
    ) -> Result<(QueryBranch, QueryHead), HttpError> {
        let JsonNewBranch {
            name,
            slug,
            start_point,
        } = branch;

        // Create branch
        let insert_branch = Self::from_json_inner(auth_conn!(context), project_id, name, slug);
        let query_branch = {
            let conn = write_conn!(context);
            conn.transaction(|conn| {
                diesel::insert_into(schema::branch::table)
                    .values(&insert_branch)
                    .execute(conn)?;
                diesel::select(last_insert_rowid()).get_result(conn)
            })
            .map_err(resource_conflict_err!(Branch, &insert_branch))
            .map(|id| insert_branch.into_query(id))?
        };
        slog::debug!(log, "Created branch {query_branch:?}");

        // Get the branch head version for the start point
        let branch_start_point = if let Some(start_point) = start_point {
            // It is okay if the start point does not exist.
            // This prevents a race condition when creating both the branch and start point in CI.
            StartPoint::from_new_json(context, project_id, start_point)
                .await
                .ok()
        } else {
            None
        };
        slog::debug!(log, "Using start point {branch_start_point:?}");

        InsertHead::for_branch(log, context, query_branch, branch_start_point.as_ref()).await
    }

    /// Convert into a [`QueryBranch`] using the given ID.
    ///
    /// Note: The returned `QueryBranch` has `head_id: None` because the head
    /// is created separately via [`InsertHead::for_branch`] after the branch insert.
    /// Callers should re-read the branch after the full transaction to get the final `head_id`.
    pub fn into_query(self, id: BranchId) -> QueryBranch {
        let Self {
            uuid,
            project_id,
            name,
            slug,
            head_id,
            created,
            modified,
            archived,
        } = self;
        QueryBranch {
            id,
            uuid,
            project_id,
            name,
            slug,
            head_id,
            created,
            modified,
            archived,
        }
    }

    fn from_json_inner(
        conn: &mut DbConnection,
        project_id: ProjectId,
        name: BranchName,
        slug: Option<BranchSlug>,
    ) -> Self {
        let slug = ok_slug!(conn, project_id, &name, slug, branch, QueryBranch);
        let timestamp = DateTime::now();
        Self {
            uuid: BranchUuid::new(),
            project_id,
            name,
            slug,
            head_id: None,
            created: timestamp,
            modified: timestamp,
            archived: None,
        }
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
    pub slug: Option<BranchSlug>,
    pub modified: DateTime,
    pub archived: Option<Option<DateTime>>,
}

impl From<JsonUpdateBranch> for UpdateBranch {
    fn from(update: JsonUpdateBranch) -> Self {
        let JsonUpdateBranch {
            name,
            slug,
            start_point: _,
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
            start_point: None,
            archived: Some(false),
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    use diesel::{Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    use bencher_json::DateTime;

    use super::BranchId;
    use crate::{
        macros::sql::last_insert_rowid,
        schema,
        test_util::{create_base_entities, setup_test_db},
    };

    #[test]
    fn last_insert_rowid_returns_branch_id() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let uuid = "00000000-0000-0000-0000-000000000010";

        let (rowid, select_id) = conn
            .transaction(|conn| {
                diesel::insert_into(schema::branch::table)
                    .values((
                        schema::branch::uuid.eq(uuid),
                        schema::branch::project_id.eq(base.project_id),
                        schema::branch::name.eq("Branch 1"),
                        schema::branch::slug.eq("branch-1"),
                        schema::branch::created.eq(DateTime::TEST),
                        schema::branch::modified.eq(DateTime::TEST),
                    ))
                    .execute(conn)?;

                let rowid = diesel::select(last_insert_rowid()).get_result::<BranchId>(conn)?;
                let select_id: BranchId = schema::branch::table
                    .filter(schema::branch::uuid.eq(uuid))
                    .select(schema::branch::id)
                    .first(conn)?;

                diesel::QueryResult::Ok((rowid, select_id))
            })
            .expect("Transaction failed");

        assert_eq!(rowid, select_id);
    }

    #[test]
    fn last_insert_rowid_matches_second_branch() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        // Insert first
        diesel::insert_into(schema::branch::table)
            .values((
                schema::branch::uuid.eq("00000000-0000-0000-0000-000000000010"),
                schema::branch::project_id.eq(base.project_id),
                schema::branch::name.eq("Branch 1"),
                schema::branch::slug.eq("branch-1"),
                schema::branch::created.eq(DateTime::TEST),
                schema::branch::modified.eq(DateTime::TEST),
            ))
            .execute(&mut conn)
            .expect("Failed to insert first branch");

        // Insert second + verify
        let second_uuid = "00000000-0000-0000-0000-000000000011";
        let (rowid, select_id) = conn
            .transaction(|conn| {
                diesel::insert_into(schema::branch::table)
                    .values((
                        schema::branch::uuid.eq(second_uuid),
                        schema::branch::project_id.eq(base.project_id),
                        schema::branch::name.eq("Branch 2"),
                        schema::branch::slug.eq("branch-2"),
                        schema::branch::created.eq(DateTime::TEST),
                        schema::branch::modified.eq(DateTime::TEST),
                    ))
                    .execute(conn)?;

                let rowid = diesel::select(last_insert_rowid()).get_result::<BranchId>(conn)?;
                let select_id: BranchId = schema::branch::table
                    .filter(schema::branch::uuid.eq(second_uuid))
                    .select(schema::branch::id)
                    .first(conn)?;

                diesel::QueryResult::Ok((rowid, select_id))
            })
            .expect("Transaction failed");

        assert_eq!(rowid, select_id);

        let first_id: BranchId = schema::branch::table
            .filter(schema::branch::uuid.eq("00000000-0000-0000-0000-000000000010"))
            .select(schema::branch::id)
            .first(&mut conn)
            .expect("Failed to get first branch id");
        assert_ne!(rowid, first_id);
    }
}
