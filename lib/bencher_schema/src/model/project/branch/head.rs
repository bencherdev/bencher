use bencher_json::{
    project::head::{JsonVersion, VersionNumber},
    BranchUuid, DateTime, GitHash, HeadUuid, JsonHead, JsonStartPoint,
};
use diesel::{
    ExpressionMethods, JoinOnDsl, NullableExpressionMethods, QueryDsl, RunQueryDsl,
    SelectableHelper,
};

use dropshot::HttpError;
use slog::Logger;

use super::{
    head_version::{HeadVersionId, InsertHeadVersion},
    start_point::StartPoint,
    version::{QueryVersion, VersionId},
    BranchId, QueryBranch,
};
use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{issue_error, resource_conflict_err, resource_not_found_err},
    macros::fn_get::fn_get,
    model::project::{
        threshold::{alert::QueryAlert, InsertThreshold},
        ProjectId,
    },
    schema::{self, head as head_table},
};

crate::macros::typed_id::typed_id!(HeadId);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = head_table)]
#[diesel(belongs_to(QueryBranch, foreign_key = branch_id))]
pub struct QueryHead {
    pub id: HeadId,
    pub uuid: HeadUuid,
    pub branch_id: BranchId,
    pub start_point_id: Option<HeadVersionId>,
    pub created: DateTime,
    pub replaced: Option<DateTime>,
}

impl QueryHead {
    fn_get!(head, HeadId);

    pub fn from_uuid(
        conn: &mut DbConnection,
        project_id: ProjectId,
        head_uuid: HeadUuid,
    ) -> Result<Self, HttpError> {
        schema::head::table
            .inner_join(schema::branch::table.on(schema::branch::id.eq(schema::head::branch_id)))
            .filter(schema::branch::project_id.eq(project_id))
            .filter(schema::head::uuid.eq(head_uuid))
            .select(Self::as_select())
            .first(conn)
            .map_err(resource_not_found_err!(Head, (project_id, head_uuid)))
    }

    pub fn get_head_json(
        conn: &mut DbConnection,
        head_id: HeadId,
        version: Option<QueryVersion>,
    ) -> Result<JsonHead, HttpError> {
        let query_head = Self::get(conn, head_id)?;

        let start_point = if let Some(start_point_id) = query_head.start_point_id {
            let (branch, head, number, hash) = schema::head_version::table
                .inner_join(
                    schema::head::table
                        .on(schema::head::id.eq(schema::head_version::head_id))
                        .inner_join(
                            schema::branch::table
                                .on(schema::branch::id.eq(schema::head::branch_id)),
                        ),
                )
                .inner_join(schema::version::table)
                .filter(schema::head_version::id.eq(start_point_id))
                .select((
                    schema::branch::uuid,
                    schema::head::uuid,
                    schema::version::number,
                    schema::version::hash.nullable(),
                ))
                .first::<(BranchUuid, HeadUuid, VersionNumber, Option<GitHash>)>(conn)
                .map_err(resource_not_found_err!(HeadVersion, start_point_id))?;

            Some(JsonStartPoint {
                branch,
                head,
                version: JsonVersion { number, hash },
            })
        } else {
            None
        };

        let Self {
            uuid,
            created,
            replaced,
            ..
        } = query_head;
        Ok(JsonHead {
            uuid,
            start_point,
            version: version.map(QueryVersion::into_json),
            created,
            replaced,
        })
    }

    pub async fn clone_start_point(
        &self,
        log: &Logger,
        context: &ApiContext,
        query_branch: &QueryBranch,
        branch_start_point: Option<&StartPoint>,
    ) -> Result<(), HttpError> {
        match (self.start_point_id, branch_start_point) {
            (Some(start_point_id), Some(branch_start_point)) => {
                debug_assert_eq!(
                    start_point_id, branch_start_point.head_version.id,
                    "Branch start point mismatch"
                );
                self.clone_versions(log, context, branch_start_point)
                    .await?;
                InsertThreshold::from_start_point(log, context, query_branch, branch_start_point)
                    .await
            },
            (None, None) => Ok(()),
            _ => Err(issue_error(
                "Branch start point mismatch",
                "Failed to match branch start point for head",
                format!("{branch_start_point:?}\n{self:?}"),
            )),
        }
    }

    async fn clone_versions(
        &self,
        log: &Logger,
        context: &ApiContext,
        branch_start_point: &StartPoint,
    ) -> Result<(), HttpError> {
        let start_point_version = QueryVersion::get(
            conn_lock!(context),
            branch_start_point.head_version.version_id,
        )?;
        slog::debug!(log, "Got start point version: {start_point_version:?}");

        // Get all prior versions (version number less than or equal to) for the start point head
        let version_ids = schema::head_version::table
            .inner_join(schema::version::table)
            .filter(schema::head_version::head_id.eq(branch_start_point.head_version.head_id))
            .filter(schema::version::number.le(start_point_version.number))
            .order(schema::version::number.desc())
            .limit(i64::from(branch_start_point.max_versions()))
            .select(schema::head_version::version_id)
            .load::<VersionId>(conn_lock!(context))
            .map_err(resource_not_found_err!(
                HeadVersion,
                (branch_start_point, start_point_version)
            ))?;
        slog::debug!(log, "Got version ids: {version_ids:?}");

        // Add new head to all start point head versions
        for version_id in version_ids {
            let insert_head_version = InsertHeadVersion {
                head_id: self.id,
                version_id,
            };
            diesel::insert_into(schema::head_version::table)
                .values(&insert_head_version)
                .execute(conn_lock!(context))
                .map_err(resource_conflict_err!(HeadVersion, insert_head_version))?;
            slog::debug!(log, "Inserted head version: {insert_head_version:?}");
        }

        slog::debug!(log, "Cloned all head versions");
        Ok(())
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = head_table)]
pub struct InsertHead {
    pub uuid: HeadUuid,
    pub branch_id: BranchId,
    pub start_point_id: Option<HeadVersionId>,
    pub created: DateTime,
    pub replaced: Option<DateTime>,
}

pub struct CloneThresholds {
    pub old_branch_id: BranchId,
    pub new_branch_id: BranchId,
}

impl InsertHead {
    #[cfg(feature = "plus")]
    pub async fn rate_limit(
        context: &ApiContext,
        query_branch: &QueryBranch,
    ) -> Result<(), HttpError> {
        use crate::{
            error::BencherResource,
            macros::rate_limit::{one_day, RateLimitError, CLAIMED_RATE_LIMIT},
        };

        let resource = BencherResource::Head;
        let (start_time, end_time) = one_day();
        let creation_count: u32 = schema::head::table
                .filter(schema::head::branch_id.eq(query_branch.id))
                .filter(schema::head::created.ge(start_time))
                .filter(schema::head::created.le(end_time))
                .count()
                .get_result::<i64>(conn_lock!(context))
                .map_err(resource_not_found_err!(Head, (query_branch, start_time, end_time)))?
                .try_into()
                .map_err(|e| {
                    issue_error(
                        "Failed to count creation",
                        &format!("Failed to count {resource} creation for branch ({uuid}) between {start_time} and {end_time}.", uuid = query_branch.uuid),
                    e
                    )}
                )?;

        // The only way that new `HEAD` can be crated is either through running a Report
        // or by updating an existing branch using the API.
        // The running of a Report will be rate limited already for unclaimed projects,
        // and the API endpoint to update an existing branch would require authentication and would therefore be a claimed project.
        if creation_count >= CLAIMED_RATE_LIMIT {
            Err(crate::error::too_many_requests(RateLimitError::Branch {
                branch: query_branch.clone(),
                resource,
            }))
        } else {
            Ok(())
        }
    }

    fn new(branch_id: BranchId, start_point_id: Option<HeadVersionId>) -> Self {
        Self {
            uuid: HeadUuid::new(),
            branch_id,
            start_point_id,
            created: DateTime::now(),
            replaced: None,
        }
    }

    pub async fn for_branch(
        log: &Logger,
        context: &ApiContext,
        query_branch: QueryBranch,
        branch_start_point: Option<&StartPoint>,
    ) -> Result<(QueryBranch, QueryHead), HttpError> {
        Self::rate_limit(context, &query_branch).await?;

        // Create the head for the branch
        let insert_head = Self::new(
            query_branch.id,
            branch_start_point.map(StartPoint::head_version_id),
        );
        diesel::insert_into(schema::head::table)
            .values(&insert_head)
            .execute(conn_lock!(context))
            .map_err(resource_conflict_err!(Head, insert_head))?;
        slog::debug!(log, "Created head: {insert_head:?}");

        // Get the new head
        let query_head = schema::head::table
            .filter(schema::head::uuid.eq(&insert_head.uuid))
            .first::<QueryHead>(conn_lock!(context))
            .map_err(resource_not_found_err!(Head, insert_head))?;
        slog::debug!(log, "Got head: {query_head:?}");

        // Update the branch head
        diesel::update(schema::branch::table.filter(schema::branch::id.eq(query_branch.id)))
            .set(schema::branch::head_id.eq(query_head.id))
            .execute(conn_lock!(context))
            .map_err(resource_conflict_err!(Branch, (&query_branch, &query_head)))?;
        slog::debug!(log, "Updated branch: {query_branch:?}");

        // If the branch has an old head, then mark it as replaced.
        // This should not run if the branch is new.
        if let Some(old_head_id) = query_branch.head_id {
            let update_head = UpdateHead::replace();
            diesel::update(schema::head::table.filter(schema::head::id.eq(old_head_id)))
                .set(&update_head)
                .execute(conn_lock!(context))
                .map_err(resource_conflict_err!(Head, (&query_branch, &update_head)))?;
            slog::debug!(log, "Updated old head to replaced: {update_head:?}");
            // Silence all alerts for the old head
            let count = QueryAlert::silence_all(context, old_head_id).await?;
            slog::debug!(log, "Silenced {count} alerts for old head");
        }

        // Get the updated branch
        // Make sure to do this after updating the old branch head to replaced
        let query_branch = QueryBranch::get(conn_lock!(context), query_branch.id)?;
        slog::debug!(log, "Got updated branch: {query_branch:?}");

        // Clone data from the start point for the head
        query_head
            .clone_start_point(log, context, &query_branch, branch_start_point)
            .await?;
        slog::debug!(
            log,
            "Cloned start point for head: {query_head:?} {branch_start_point:?}"
        );

        Ok((query_branch, query_head))
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = head_table)]
pub struct UpdateHead {
    pub replaced: DateTime,
}

impl UpdateHead {
    pub fn replace() -> Self {
        Self {
            replaced: DateTime::now(),
        }
    }
}
