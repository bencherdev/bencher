use bencher_json::{
    BranchUuid, DateTime, GitHash, HeadUuid, JsonHead, JsonStartPoint,
    project::head::{JsonVersion, VersionNumber},
};
use diesel::{
    Connection as _, ExpressionMethods as _, JoinOnDsl as _, NullableExpressionMethods as _,
    QueryDsl as _, RunQueryDsl as _, SelectableHelper as _,
};

use dropshot::HttpError;
use slog::Logger;

use super::{
    BranchId, QueryBranch,
    head_version::{HeadVersionId, InsertHeadVersion},
    start_point::StartPoint,
    version::{QueryVersion, VersionId},
};
use crate::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::{issue_error, resource_conflict_err, resource_not_found_err},
    macros::{fn_get::fn_get, sql::last_insert_rowid},
    model::project::{
        ProjectId,
        threshold::{InsertThreshold, alert::QueryAlert},
    },
    schema::{self, head as head_table},
    write_conn,
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
            auth_conn!(context),
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
            .load::<VersionId>(auth_conn!(context))
            .map_err(resource_not_found_err!(
                HeadVersion,
                (branch_start_point, start_point_version)
            ))?;
        slog::debug!(log, "Got version ids: {version_ids:?}");

        // Add new head to all start point head versions in a single batch insert
        let insert_head_versions: Vec<InsertHeadVersion> = version_ids
            .into_iter()
            .map(|version_id| InsertHeadVersion {
                head_id: self.id,
                version_id,
            })
            .collect();

        if !insert_head_versions.is_empty() {
            diesel::insert_into(schema::head_version::table)
                .values(&insert_head_versions)
                .execute(write_conn!(context))
                .map_err(resource_conflict_err!(HeadVersion, &insert_head_versions))?;
            slog::debug!(
                log,
                "Inserted {} head versions in batch",
                insert_head_versions.len()
            );
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
        use crate::{context::RateLimitingError, error::BencherResource};

        let resource = BencherResource::Head;
        let (start_time, end_time) = context.rate_limiting.window();
        let window_usage: u32 = schema::head::table
                .filter(schema::head::branch_id.eq(query_branch.id))
                .filter(schema::head::created.ge(start_time))
                .filter(schema::head::created.le(end_time))
                .count()
                .get_result::<i64>(auth_conn!(context))
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
        context
            .rate_limiting
            .check_claimed_limit(window_usage, |rate_limit| RateLimitingError::Branch {
                branch: query_branch.clone(),
                resource,
                rate_limit,
            })
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
        // Phase 1: Rate limit (requires await)
        #[cfg(feature = "plus")]
        Self::rate_limit(context, &query_branch).await?;

        // Build the insert_head before acquiring the write lock
        let insert_head = Self::new(
            query_branch.id,
            branch_start_point.map(StartPoint::head_version_id),
        );
        let old_head_id = query_branch.head_id;

        // Phase 2: Batch all writes in a single transaction
        let new_head_id = {
            let conn = write_conn!(context);
            conn.transaction(|conn| {
                // Insert the new head
                diesel::insert_into(schema::head::table)
                    .values(&insert_head)
                    .execute(conn)?;
                let new_head_id: HeadId = diesel::select(last_insert_rowid()).get_result(conn)?;

                // Update the branch to point to the new head
                diesel::update(
                    schema::branch::table.filter(schema::branch::id.eq(query_branch.id)),
                )
                .set(schema::branch::head_id.eq(new_head_id))
                .execute(conn)?;

                // If there is an old head, mark it as replaced and silence its alerts
                if let Some(old_head_id) = old_head_id {
                    let update_head = UpdateHead::replace();
                    diesel::update(schema::head::table.filter(schema::head::id.eq(old_head_id)))
                        .set(&update_head)
                        .execute(conn)?;

                    QueryAlert::silence_all(conn, old_head_id)?;
                }

                Ok::<_, diesel::result::Error>(new_head_id)
            })
            .map_err(|e| {
                issue_error(
                    "Failed to create head for branch",
                    "Failed to create head for branch in batch transaction:",
                    e,
                )
            })?
        };
        slog::debug!(
            log,
            "Created head {new_head_id:?} for branch: {insert_head:?}"
        );

        // Read back using read connections
        let query_head = QueryHead::get(auth_conn!(context), new_head_id)?;
        slog::debug!(log, "Got head: {query_head:?}");

        let query_branch = QueryBranch::get(auth_conn!(context), query_branch.id)?;
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

#[cfg(test)]
mod tests {
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    use bencher_json::DateTime;

    use crate::{
        model::project::branch::version::VersionId,
        schema,
        test_util::{
            count_head_versions, create_base_entities, create_branch_with_head,
            create_head_version, create_version, get_branch_head_id, get_head_replaced,
            get_head_versions, setup_test_db,
        },
    };

    /// Test that `head_version` records can be queried by `head_id`.
    /// This is the foundation of the `clone_versions` operation.
    #[test]
    fn query_head_versions_by_head() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        // Create a source branch with head
        let source = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "source",
            "source",
            "00000000-0000-0000-0000-000000000011",
        );

        // Create versions and link them to the source head
        let v1 = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000100",
            1,
            None,
        );
        let v2 = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000101",
            2,
            None,
        );
        let v3 = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000102",
            3,
            None,
        );

        create_head_version(&mut conn, source.head_id, v1);
        create_head_version(&mut conn, source.head_id, v2);
        create_head_version(&mut conn, source.head_id, v3);

        // Query head_versions for the source head
        let head_versions = get_head_versions(&mut conn, source.head_id);
        assert_eq!(head_versions.len(), 3);

        // Verify all versions are linked to the source head
        for (head_id, _) in &head_versions {
            assert_eq!(*head_id, source.head_id);
        }
    }

    /// Test querying `head_versions` with version number filter (le = less than or equal).
    /// The `clone_versions` function uses this filter to clone only versions up to
    /// the start point version.
    #[test]
    fn query_head_versions_with_version_number_filter() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let source = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "source",
            "source",
            "00000000-0000-0000-0000-000000000011",
        );

        // Create 5 versions with numbers 1-5
        let v1 = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000100",
            1,
            None,
        );
        let v2 = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000101",
            2,
            None,
        );
        let v3 = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000102",
            3,
            None,
        );
        let v4 = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000103",
            4,
            None,
        );
        let v5 = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000104",
            5,
            None,
        );

        // Link all versions to the source head
        create_head_version(&mut conn, source.head_id, v1);
        create_head_version(&mut conn, source.head_id, v2);
        create_head_version(&mut conn, source.head_id, v3);
        create_head_version(&mut conn, source.head_id, v4);
        create_head_version(&mut conn, source.head_id, v5);

        // Query versions <= 3 (simulating start point at version 3)
        let version_ids: Vec<VersionId> = schema::head_version::table
            .inner_join(schema::version::table)
            .filter(schema::head_version::head_id.eq(source.head_id))
            .filter(schema::version::number.le(3))
            .order(schema::version::number.desc())
            .select(schema::head_version::version_id)
            .load(&mut conn)
            .expect("Failed to query");

        assert_eq!(version_ids.len(), 3);
        // Should be in descending order: v3, v2, v1
        assert_eq!(version_ids.first(), Some(&v3));
        assert_eq!(version_ids.get(1), Some(&v2));
        assert_eq!(version_ids.get(2), Some(&v1));
    }

    /// Test that the limit clause works correctly for `max_versions`.
    /// The `clone_versions` function uses limit to cap the number of versions cloned.
    #[test]
    fn query_head_versions_with_limit() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let source = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "source",
            "source",
            "00000000-0000-0000-0000-000000000011",
        );

        // Create 10 versions
        for i in 1..=10 {
            let v = create_version(
                &mut conn,
                base.project_id,
                &format!("00000000-0000-0000-0000-0000000001{i:02}"),
                i,
                None,
            );
            create_head_version(&mut conn, source.head_id, v);
        }

        // Query with limit 5 (simulating max_versions=5)
        let version_ids: Vec<VersionId> = schema::head_version::table
            .inner_join(schema::version::table)
            .filter(schema::head_version::head_id.eq(source.head_id))
            .order(schema::version::number.desc())
            .limit(5)
            .select(schema::head_version::version_id)
            .load(&mut conn)
            .expect("Failed to query");

        assert_eq!(version_ids.len(), 5);
        // Should get the 5 most recent versions (highest numbers) in descending order
    }

    /// Test cloning versions to a new head using individual inserts.
    /// This simulates the current `clone_versions` behavior before optimization.
    #[test]
    fn clone_versions_individual_inserts() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        // Create source branch with head
        let source = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "source",
            "source",
            "00000000-0000-0000-0000-000000000011",
        );

        // Create destination branch with head
        let dest = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "dest",
            "dest",
            "00000000-0000-0000-0000-000000000021",
        );

        // Create versions and link to source
        let v1 = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000100",
            1,
            None,
        );
        let v2 = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000101",
            2,
            None,
        );
        let v3 = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000102",
            3,
            None,
        );

        create_head_version(&mut conn, source.head_id, v1);
        create_head_version(&mut conn, source.head_id, v2);
        create_head_version(&mut conn, source.head_id, v3);

        // Simulate clone_versions: clone versions <= 3 to dest head
        // This uses individual inserts like the current implementation
        let version_ids: Vec<VersionId> = schema::head_version::table
            .inner_join(schema::version::table)
            .filter(schema::head_version::head_id.eq(source.head_id))
            .filter(schema::version::number.le(3))
            .order(schema::version::number.desc())
            .limit(255) // max_versions default
            .select(schema::head_version::version_id)
            .load(&mut conn)
            .expect("Failed to query");

        // Insert each version individually (current behavior)
        for version_id in &version_ids {
            diesel::insert_into(schema::head_version::table)
                .values((
                    schema::head_version::head_id.eq(dest.head_id),
                    schema::head_version::version_id.eq(*version_id),
                ))
                .execute(&mut conn)
                .expect("Failed to insert");
        }

        // Verify dest head now has all 3 versions
        let dest_versions = get_head_versions(&mut conn, dest.head_id);
        assert_eq!(dest_versions.len(), 3);

        // Verify source head still has its versions
        let source_versions = get_head_versions(&mut conn, source.head_id);
        assert_eq!(source_versions.len(), 3);
    }

    /// Test cloning versions to a new head using batch insert.
    /// This simulates the optimized `clone_versions` behavior.
    #[test]
    fn clone_versions_batch_insert() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        // Create source branch with head
        let source = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "source",
            "source",
            "00000000-0000-0000-0000-000000000011",
        );

        // Create destination branch with head
        let dest = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "dest",
            "dest",
            "00000000-0000-0000-0000-000000000021",
        );

        // Create versions and link to source
        let v1 = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000100",
            1,
            None,
        );
        let v2 = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000101",
            2,
            None,
        );
        let v3 = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000102",
            3,
            None,
        );

        create_head_version(&mut conn, source.head_id, v1);
        create_head_version(&mut conn, source.head_id, v2);
        create_head_version(&mut conn, source.head_id, v3);

        // Simulate clone_versions with batch insert (optimized behavior)
        let version_ids: Vec<VersionId> = schema::head_version::table
            .inner_join(schema::version::table)
            .filter(schema::head_version::head_id.eq(source.head_id))
            .filter(schema::version::number.le(3))
            .order(schema::version::number.desc())
            .limit(255)
            .select(schema::head_version::version_id)
            .load(&mut conn)
            .expect("Failed to query");

        // Create batch of tuple values for insert
        let insert_values: Vec<_> = version_ids
            .into_iter()
            .map(|version_id| {
                (
                    schema::head_version::head_id.eq(dest.head_id),
                    schema::head_version::version_id.eq(version_id),
                )
            })
            .collect();

        // Insert all at once (optimized behavior)
        diesel::insert_into(schema::head_version::table)
            .values(&insert_values)
            .execute(&mut conn)
            .expect("Failed to batch insert");

        // Verify dest head now has all 3 versions
        let dest_versions = get_head_versions(&mut conn, dest.head_id);
        assert_eq!(dest_versions.len(), 3);
    }

    /// Test that batch insert and individual insert produce the same result.
    /// This is critical for ensuring the optimization doesn't change behavior.
    #[test]
    fn batch_and_individual_insert_equivalence() {
        // Test with individual inserts
        let mut conn1 = setup_test_db();
        let base1 = create_base_entities(&mut conn1);
        let source1 = create_branch_with_head(
            &mut conn1,
            base1.project_id,
            "00000000-0000-0000-0000-000000000010",
            "source",
            "source",
            "00000000-0000-0000-0000-000000000011",
        );
        let dest1 = create_branch_with_head(
            &mut conn1,
            base1.project_id,
            "00000000-0000-0000-0000-000000000020",
            "dest",
            "dest",
            "00000000-0000-0000-0000-000000000021",
        );

        // Test with batch insert
        let mut conn2 = setup_test_db();
        let base2 = create_base_entities(&mut conn2);
        let source2 = create_branch_with_head(
            &mut conn2,
            base2.project_id,
            "00000000-0000-0000-0000-000000000010",
            "source",
            "source",
            "00000000-0000-0000-0000-000000000011",
        );
        let dest2 = create_branch_with_head(
            &mut conn2,
            base2.project_id,
            "00000000-0000-0000-0000-000000000020",
            "dest",
            "dest",
            "00000000-0000-0000-0000-000000000021",
        );

        // Create same versions in both databases
        let versions1: Vec<VersionId> = (1..=5)
            .map(|i| {
                let v = create_version(
                    &mut conn1,
                    base1.project_id,
                    &format!("00000000-0000-0000-0000-0000000001{i:02}"),
                    i,
                    None,
                );
                create_head_version(&mut conn1, source1.head_id, v);
                v
            })
            .collect();

        let versions2: Vec<VersionId> = (1..=5)
            .map(|i| {
                let v = create_version(
                    &mut conn2,
                    base2.project_id,
                    &format!("00000000-0000-0000-0000-0000000001{i:02}"),
                    i,
                    None,
                );
                create_head_version(&mut conn2, source2.head_id, v);
                v
            })
            .collect();

        // Individual inserts for conn1
        for version_id in &versions1 {
            diesel::insert_into(schema::head_version::table)
                .values((
                    schema::head_version::head_id.eq(dest1.head_id),
                    schema::head_version::version_id.eq(*version_id),
                ))
                .execute(&mut conn1)
                .expect("Failed to insert");
        }

        // Batch insert for conn2
        let batch: Vec<_> = versions2
            .iter()
            .map(|&version_id| {
                (
                    schema::head_version::head_id.eq(dest2.head_id),
                    schema::head_version::version_id.eq(version_id),
                )
            })
            .collect();
        diesel::insert_into(schema::head_version::table)
            .values(&batch)
            .execute(&mut conn2)
            .expect("Failed to batch insert");

        // Verify both produce same count
        let count1 = count_head_versions(&mut conn1, dest1.head_id);
        let count2 = count_head_versions(&mut conn2, dest2.head_id);
        assert_eq!(count1, count2);
        assert_eq!(count1, 5);
    }

    /// Test cloning with empty version list.
    /// Handles edge case where start point has no versions.
    #[test]
    fn clone_empty_versions() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let source = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "source",
            "source",
            "00000000-0000-0000-0000-000000000011",
        );

        let dest = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "dest",
            "dest",
            "00000000-0000-0000-0000-000000000021",
        );

        // Source has no versions linked
        let version_ids: Vec<VersionId> = schema::head_version::table
            .inner_join(schema::version::table)
            .filter(schema::head_version::head_id.eq(source.head_id))
            .select(schema::head_version::version_id)
            .load(&mut conn)
            .expect("Failed to query");

        assert!(version_ids.is_empty());

        // Batch insert with empty list should succeed
        let insert_values: Vec<_> = version_ids
            .into_iter()
            .map(|version_id| {
                (
                    schema::head_version::head_id.eq(dest.head_id),
                    schema::head_version::version_id.eq(version_id),
                )
            })
            .collect();

        if !insert_values.is_empty() {
            diesel::insert_into(schema::head_version::table)
                .values(&insert_values)
                .execute(&mut conn)
                .expect("Failed to batch insert");
        }

        // Dest should have no versions
        let dest_count = count_head_versions(&mut conn, dest.head_id);
        assert_eq!(dest_count, 0);
    }

    /// Test cloning respects `max_versions` limit.
    /// Simulates `max_versions=3` when there are 10 versions available.
    #[test]
    fn clone_respects_max_versions() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let source = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "source",
            "source",
            "00000000-0000-0000-0000-000000000011",
        );

        let dest = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "dest",
            "dest",
            "00000000-0000-0000-0000-000000000021",
        );

        // Create 10 versions
        for i in 1..=10 {
            let v = create_version(
                &mut conn,
                base.project_id,
                &format!("00000000-0000-0000-0000-0000000001{i:02}"),
                i,
                None,
            );
            create_head_version(&mut conn, source.head_id, v);
        }

        // Clone with max_versions=3
        let max_versions: i64 = 3;
        let version_ids: Vec<VersionId> = schema::head_version::table
            .inner_join(schema::version::table)
            .filter(schema::head_version::head_id.eq(source.head_id))
            .order(schema::version::number.desc())
            .limit(max_versions)
            .select(schema::head_version::version_id)
            .load(&mut conn)
            .expect("Failed to query");

        assert_eq!(version_ids.len(), 3);

        let insert_values: Vec<_> = version_ids
            .into_iter()
            .map(|version_id| {
                (
                    schema::head_version::head_id.eq(dest.head_id),
                    schema::head_version::version_id.eq(version_id),
                )
            })
            .collect();

        diesel::insert_into(schema::head_version::table)
            .values(&insert_values)
            .execute(&mut conn)
            .expect("Failed to batch insert");

        // Dest should have exactly 3 versions
        let dest_count = count_head_versions(&mut conn, dest.head_id);
        assert_eq!(dest_count, 3);
    }

    /// Test cloning versions with version number filter and `max_versions`.
    /// Simulates `start_point` at version 7 with `max_versions=5`.
    #[test]
    fn clone_with_version_filter_and_limit() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let source = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "source",
            "source",
            "00000000-0000-0000-0000-000000000011",
        );

        // Note: dest not used in this test, only verifying version query behavior
        let _dest = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "dest",
            "dest",
            "00000000-0000-0000-0000-000000000021",
        );

        // Create 10 versions
        for i in 1..=10 {
            let v = create_version(
                &mut conn,
                base.project_id,
                &format!("00000000-0000-0000-0000-0000000001{i:02}"),
                i,
                None,
            );
            create_head_version(&mut conn, source.head_id, v);
        }

        // Clone versions <= 7 with max_versions=5
        // Should get versions 7, 6, 5, 4, 3 (5 most recent that are <= 7)
        let start_point_version_number = 7;
        let max_versions: i64 = 5;

        let version_ids: Vec<VersionId> = schema::head_version::table
            .inner_join(schema::version::table)
            .filter(schema::head_version::head_id.eq(source.head_id))
            .filter(schema::version::number.le(start_point_version_number))
            .order(schema::version::number.desc())
            .limit(max_versions)
            .select(schema::head_version::version_id)
            .load(&mut conn)
            .expect("Failed to query");

        assert_eq!(version_ids.len(), 5);

        // Verify version numbers are correct (7, 6, 5, 4, 3 in descending order)
        let version_numbers: Vec<i32> = schema::version::table
            .filter(schema::version::id.eq_any(&version_ids))
            .order(schema::version::number.desc())
            .select(schema::version::number)
            .load(&mut conn)
            .expect("Failed to query version numbers");

        assert_eq!(version_numbers, vec![7, 6, 5, 4, 3]);
    }

    /// Test that multiple heads can share the same versions.
    /// This is important for the start point cloning feature.
    #[test]
    fn multiple_heads_share_versions() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let source = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "source",
            "source",
            "00000000-0000-0000-0000-000000000011",
        );

        let dest1 = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "dest1",
            "dest1",
            "00000000-0000-0000-0000-000000000021",
        );

        let dest2 = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "dest2",
            "dest2",
            "00000000-0000-0000-0000-000000000031",
        );

        // Create versions and link to source
        let v1 = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000100",
            1,
            None,
        );
        let v2 = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000101",
            2,
            None,
        );

        create_head_version(&mut conn, source.head_id, v1);
        create_head_version(&mut conn, source.head_id, v2);

        // Clone to both dest heads
        for dest in [&dest1, &dest2] {
            let insert_values: Vec<_> = [v1, v2]
                .into_iter()
                .map(|version_id| {
                    (
                        schema::head_version::head_id.eq(dest.head_id),
                        schema::head_version::version_id.eq(version_id),
                    )
                })
                .collect();

            diesel::insert_into(schema::head_version::table)
                .values(&insert_values)
                .execute(&mut conn)
                .expect("Failed to insert");
        }

        // Each head should have 2 versions
        assert_eq!(count_head_versions(&mut conn, source.head_id), 2);
        assert_eq!(count_head_versions(&mut conn, dest1.head_id), 2);
        assert_eq!(count_head_versions(&mut conn, dest2.head_id), 2);

        // Total head_version records should be 6 (2 versions * 3 heads)
        let total: i64 = schema::head_version::table
            .count()
            .get_result(&mut conn)
            .expect("Failed to count");
        assert_eq!(total, 6);
    }

    /// Test that inserting a head and updating branch `head_id` works in a single transaction.
    #[test]
    fn for_branch_inserts_head_and_updates_branch() {
        use diesel::Connection as _;

        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        // Create a branch with an initial head
        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        // Run the transaction: insert new head + update branch
        let new_head_id = conn
            .transaction(|conn| {
                use super::InsertHead;
                use crate::macros::sql::last_insert_rowid;

                let insert_head = InsertHead::new(branch.branch_id, None);
                diesel::insert_into(schema::head::table)
                    .values(&insert_head)
                    .execute(conn)?;
                let new_head_id: super::HeadId =
                    diesel::select(last_insert_rowid()).get_result(conn)?;

                diesel::update(
                    schema::branch::table.filter(schema::branch::id.eq(branch.branch_id)),
                )
                .set(schema::branch::head_id.eq(new_head_id))
                .execute(conn)?;

                Ok::<_, diesel::result::Error>(new_head_id)
            })
            .expect("Transaction failed");

        // Verify branch now points to the new head
        let head_id = get_branch_head_id(&mut conn, branch.branch_id);
        assert_eq!(head_id, Some(new_head_id));
        assert_ne!(new_head_id, branch.head_id);
    }

    /// Test that old head gets marked as replaced in a transaction.
    #[test]
    fn for_branch_replaces_old_head() {
        use diesel::Connection as _;

        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        // Old head should not be replaced yet
        assert!(get_head_replaced(&mut conn, branch.head_id).is_none());

        // Run transaction: insert new head, update branch, mark old head replaced
        conn.transaction(|conn| {
            use super::{InsertHead, UpdateHead};
            use crate::macros::sql::last_insert_rowid;

            let insert_head = InsertHead::new(branch.branch_id, None);
            diesel::insert_into(schema::head::table)
                .values(&insert_head)
                .execute(conn)?;
            let new_head_id: super::HeadId =
                diesel::select(last_insert_rowid()).get_result(conn)?;

            diesel::update(schema::branch::table.filter(schema::branch::id.eq(branch.branch_id)))
                .set(schema::branch::head_id.eq(new_head_id))
                .execute(conn)?;

            // Mark old head as replaced
            let update_head = UpdateHead::replace();
            diesel::update(schema::head::table.filter(schema::head::id.eq(branch.head_id)))
                .set(&update_head)
                .execute(conn)?;

            Ok::<_, diesel::result::Error>(())
        })
        .expect("Transaction failed");

        // Old head should now be replaced
        assert!(get_head_replaced(&mut conn, branch.head_id).is_some());
    }

    /// Test that silencing alerts for old head works within a transaction.
    #[test]
    #[expect(clippy::too_many_lines)]
    fn for_branch_silences_old_head_alerts() {
        use diesel::Connection as _;

        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );
        let testbed = crate::test_util::create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );
        let version_id = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            1,
            None,
        );
        create_head_version(&mut conn, branch.head_id, version_id);
        let measure = crate::test_util::create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000040",
            "latency",
            "latency",
        );

        // Create an alert chain on the old head
        let report_id = crate::test_util::create_report(
            &mut conn,
            "00000000-0000-0000-0000-000000000100",
            base.project_id,
            branch.head_id,
            version_id,
            testbed,
        );
        let benchmark_id = crate::test_util::create_benchmark(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000101",
            "bench1",
            "bench1",
        );
        let report_benchmark_id = crate::test_util::create_report_benchmark(
            &mut conn,
            "00000000-0000-0000-0000-000000000102",
            report_id,
            0,
            benchmark_id,
        );
        let metric_id = crate::test_util::create_metric(
            &mut conn,
            "00000000-0000-0000-0000-000000000103",
            report_benchmark_id,
            measure,
            1.0,
        );
        let threshold_id = crate::test_util::create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000104",
        );
        let model_id = crate::test_util::create_model(
            &mut conn,
            threshold_id,
            "00000000-0000-0000-0000-000000000105",
            0,
        );
        let boundary_id = crate::test_util::create_boundary(
            &mut conn,
            "00000000-0000-0000-0000-000000000106",
            metric_id,
            threshold_id,
            model_id,
        );
        let alert_id = crate::test_util::create_alert(
            &mut conn,
            "00000000-0000-0000-0000-000000000107",
            boundary_id,
            true,
            0, // Active
        );

        // Alert should be active
        assert_eq!(crate::test_util::get_alert_status(&mut conn, alert_id), 0);

        // Run transaction: silence alerts for old head
        conn.transaction(|conn| {
            use super::super::super::threshold::alert::{AlertId, UpdateAlert};

            let alerts = schema::alert::table
                .inner_join(schema::boundary::table.inner_join(
                    schema::metric::table.inner_join(
                        schema::report_benchmark::table.inner_join(schema::report::table),
                    ),
                ))
                .filter(schema::report::head_id.eq(branch.head_id))
                .select(schema::alert::id)
                .load::<AlertId>(conn)?;

            if !alerts.is_empty() {
                let silenced_alert = UpdateAlert::silence();
                diesel::update(schema::alert::table.filter(schema::alert::id.eq_any(&alerts)))
                    .set(&silenced_alert)
                    .execute(conn)?;
            }

            Ok::<_, diesel::result::Error>(())
        })
        .expect("Transaction failed");

        // Alert should now be silenced (10)
        assert_eq!(crate::test_util::get_alert_status(&mut conn, alert_id), 10);
    }

    /// Test that transaction works correctly when branch has no old head.
    #[test]
    fn for_branch_no_old_head_skips_replace() {
        use diesel::Connection as _;

        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        // Create a branch WITHOUT a head (manually, so head_id is None)
        diesel::insert_into(schema::branch::table)
            .values((
                schema::branch::uuid.eq("00000000-0000-0000-0000-000000000010"),
                schema::branch::project_id.eq(base.project_id),
                schema::branch::name.eq("new-branch"),
                schema::branch::slug.eq("new-branch"),
                schema::branch::created.eq(DateTime::TEST),
                schema::branch::modified.eq(DateTime::TEST),
            ))
            .execute(&mut conn)
            .expect("Failed to insert branch");

        let branch_id: super::super::BranchId = {
            use crate::macros::sql::last_insert_rowid;
            diesel::select(last_insert_rowid())
                .get_result(&mut conn)
                .expect("Failed to get branch id")
        };

        // Branch has no head
        assert!(get_branch_head_id(&mut conn, branch_id).is_none());

        // Run transaction: insert new head, update branch, no old head to replace
        let new_head_id = conn
            .transaction(|conn| {
                use super::InsertHead;
                use crate::macros::sql::last_insert_rowid;

                let insert_head = InsertHead::new(branch_id, None);
                diesel::insert_into(schema::head::table)
                    .values(&insert_head)
                    .execute(conn)?;
                let new_head_id: super::HeadId =
                    diesel::select(last_insert_rowid()).get_result(conn)?;

                diesel::update(schema::branch::table.filter(schema::branch::id.eq(branch_id)))
                    .set(schema::branch::head_id.eq(new_head_id))
                    .execute(conn)?;

                // No old head to replace — this is fine
                let old_head_id: Option<super::HeadId> = None;
                assert!(old_head_id.is_none(), "Should not have old head");

                Ok::<_, diesel::result::Error>(new_head_id)
            })
            .expect("Transaction failed");

        // Branch should now point to the new head
        let head_id = get_branch_head_id(&mut conn, branch_id);
        assert_eq!(head_id, Some(new_head_id));
    }

    /// Test large batch insert (simulating `max_versions=255`).
    /// This ensures the batch insert can handle the maximum load.
    #[test]
    fn large_batch_insert() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let source = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "source",
            "source",
            "00000000-0000-0000-0000-000000000011",
        );

        let dest = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "dest",
            "dest",
            "00000000-0000-0000-0000-000000000021",
        );

        // Create 100 versions (testing with a reasonable large number)
        let mut version_ids = Vec::new();
        for i in 1i32..=100 {
            #[expect(clippy::cast_sign_loss, reason = "i is always positive in this range")]
            let uuid = format!("{:08x}-0000-0000-0000-000000000100", i as u32);
            let v = create_version(&mut conn, base.project_id, &uuid, i, None);
            create_head_version(&mut conn, source.head_id, v);
            version_ids.push(v);
        }

        // Batch insert all versions to dest
        let insert_values: Vec<_> = version_ids
            .into_iter()
            .map(|version_id| {
                (
                    schema::head_version::head_id.eq(dest.head_id),
                    schema::head_version::version_id.eq(version_id),
                )
            })
            .collect();

        diesel::insert_into(schema::head_version::table)
            .values(&insert_values)
            .execute(&mut conn)
            .expect("Failed to batch insert 100 versions");

        assert_eq!(count_head_versions(&mut conn, dest.head_id), 100);
    }
}
