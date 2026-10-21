use bencher_json::{
    BranchUuid, DateTime, GitHash, HeadIdempotencyKey, HeadUuid, JsonHead, JsonStartPoint,
    project::head::{JsonVersion, VersionNumber},
};
use diesel::{
    ExpressionMethods as _, JoinOnDsl as _, NullableExpressionMethods as _, OptionalExtension as _,
    QueryDsl as _, RunQueryDsl as _, SelectableHelper as _,
};

use dropshot::HttpError;
use slog::Logger;

use super::{
    BranchId, QueryBranch,
    head_version::{HeadVersionId, InsertHeadVersion},
    start_point::StartPoint,
    version::QueryVersion,
};
use crate::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::{issue_error, resource_not_found_err},
    macros::{fn_get::fn_get, sql::last_insert_rowid},
    model::project::{
        ProjectId,
        report::warning::ReportWarnings,
        threshold::{InsertThreshold, alert::QueryAlert},
    },
    schema::{self, head as head_table},
    write_transaction,
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
    pub idempotency_key: Option<HeadIdempotencyKey>,
    pub created: DateTime,
    pub replaced: Option<DateTime>,
}

impl QueryHead {
    fn_get!(head, HeadId);

    /// The head of the branch that holds the idempotency key, replaced or not.
    fn id_for_idempotency_key(
        conn: &mut DbConnection,
        branch_id: BranchId,
        idempotency_key: HeadIdempotencyKey,
    ) -> diesel::QueryResult<Option<HeadId>> {
        schema::head::table
            .filter(schema::head::branch_id.eq(branch_id))
            .filter(schema::head::idempotency_key.eq(idempotency_key))
            .select(schema::head::id)
            .first(conn)
            .optional()
    }

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
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = head_table)]
pub struct InsertHead {
    pub uuid: HeadUuid,
    pub branch_id: BranchId,
    pub start_point_id: Option<HeadVersionId>,
    pub idempotency_key: Option<HeadIdempotencyKey>,
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

    pub fn new(branch_id: BranchId, start_point_id: Option<HeadVersionId>) -> Self {
        Self::new_with_idempotency_key(branch_id, start_point_id, None)
    }

    pub fn new_with_idempotency_key(
        branch_id: BranchId,
        start_point_id: Option<HeadVersionId>,
        idempotency_key: Option<HeadIdempotencyKey>,
    ) -> Self {
        Self {
            uuid: HeadUuid::new(),
            branch_id,
            start_point_id,
            idempotency_key,
            created: DateTime::now(),
            replaced: None,
        }
    }

    pub async fn for_branch(
        log: &Logger,
        context: &ApiContext,
        query_branch: QueryBranch,
        branch_start_point: Option<&StartPoint>,
        warnings: &mut ReportWarnings,
    ) -> Result<(QueryBranch, QueryHead), HttpError> {
        Self::for_branch_with_idempotency_key(
            log,
            context,
            query_branch,
            branch_start_point,
            None,
            warnings,
        )
        .await
    }

    /// A new head for the branch, or the head of the branch that already holds the idempotency key.
    pub async fn for_branch_with_idempotency_key(
        log: &Logger,
        context: &ApiContext,
        query_branch: QueryBranch,
        branch_start_point: Option<&StartPoint>,
        idempotency_key: Option<HeadIdempotencyKey>,
        warnings: &mut ReportWarnings,
    ) -> Result<(QueryBranch, QueryHead), HttpError> {
        // A head that already holds the key is found before the rate limit,
        // so reusing it never counts as a new head.
        let held_head_id = if let Some(idempotency_key) = idempotency_key {
            QueryHead::id_for_idempotency_key(auth_conn!(context), query_branch.id, idempotency_key)
                .map_err(|e| {
                    issue_error(
                        "Failed to check head idempotency key",
                        "Failed to check head idempotency key for branch:",
                        e,
                    )
                })?
        } else {
            None
        };

        let head_id = if let Some(head_id) = held_head_id {
            slog::debug!(
                log,
                "Reusing head {head_id:?} with idempotency key {idempotency_key:?}"
            );
            head_id
        } else {
            #[cfg(feature = "plus")]
            Self::rate_limit(context, &query_branch).await?;

            let insert_head = Self::new_with_idempotency_key(
                query_branch.id,
                branch_start_point.map(StartPoint::head_version_id),
                idempotency_key,
            );
            let now = context.clock.now();
            let branch_head = write_transaction!(context, |conn| {
                insert_head.insert_for_branch(conn, branch_start_point, now)
            })
            .map_err(|e| {
                issue_error(
                    "Failed to create head for branch",
                    "Failed to create head for branch in batch transaction:",
                    e,
                )
            })?;
            match branch_head {
                BranchHead::Held(head_id) => {
                    slog::debug!(
                        log,
                        "Reusing head {head_id:?} with idempotency key {idempotency_key:?}"
                    );
                    head_id
                },
                BranchHead::Created {
                    head_id,
                    silenced_alerts,
                } => {
                    slog::debug!(
                        log,
                        "Created head {head_id:?} for branch: {insert_head:?} (silenced {silenced_alerts} alerts)"
                    );
                    head_id
                },
            }
        };

        // Read back using read connections
        let query_head = QueryHead::get(auth_conn!(context), head_id)?;
        slog::debug!(log, "Got head: {query_head:?}");

        let query_branch = QueryBranch::get(auth_conn!(context), query_branch.id)?;
        slog::debug!(log, "Got updated branch: {query_branch:?}");

        if let Some(start_point) = branch_start_point {
            InsertThreshold::from_start_point(log, context, &query_branch, start_point, warnings)
                .await?;
        }

        Ok((query_branch, query_head))
    }

    /// Point the branch at this new head, cloning the start point versions,
    /// and replace the head the branch names when the write runs.
    /// If a head of the branch already holds the idempotency key,
    /// the branch is left as it is and that head is returned.
    fn insert_for_branch(
        &self,
        conn: &mut DbConnection,
        branch_start_point: Option<&StartPoint>,
        now: DateTime,
    ) -> diesel::QueryResult<BranchHead> {
        if let Some(idempotency_key) = self.idempotency_key
            && let Some(head_id) =
                QueryHead::id_for_idempotency_key(conn, self.branch_id, idempotency_key)?
        {
            return Ok(BranchHead::Held(head_id));
        }

        // The head to replace is read under the write lock,
        // so overlapping resets each replace the head they follow.
        let old_head_id: Option<HeadId> = schema::branch::table
            .filter(schema::branch::id.eq(self.branch_id))
            .select(schema::branch::head_id)
            .first(conn)?;

        // Insert the new head
        diesel::insert_into(schema::head::table)
            .values(self)
            .execute(conn)?;
        let head_id: HeadId = diesel::select(last_insert_rowid()).get_result(conn)?;
        let version_ids = if let Some(start_point) = branch_start_point {
            start_point.version_ids(conn)?
        } else {
            Vec::new()
        };
        InsertHeadVersion::insert_all(conn, head_id, &version_ids)?;

        // Update the branch to point to the new head
        diesel::update(schema::branch::table.filter(schema::branch::id.eq(self.branch_id)))
            .set(schema::branch::head_id.eq(head_id))
            .execute(conn)?;

        // If there is an old head, mark it as replaced and silence its alerts
        let silenced_alerts = if let Some(old_head_id) = old_head_id {
            let update_head = UpdateHead::replace();
            diesel::update(schema::head::table.filter(schema::head::id.eq(old_head_id)))
                .set(&update_head)
                .execute(conn)?;

            QueryAlert::silence_all(conn, old_head_id, now)?
        } else {
            0
        };

        Ok(BranchHead::Created {
            head_id,
            silenced_alerts,
        })
    }
}

/// The head a branch uses after [`InsertHead::insert_for_branch`].
#[derive(Debug, Clone, Copy)]
enum BranchHead {
    /// A head of the branch already holds the idempotency key.
    Held(HeadId),
    /// The new head, with the count of alerts silenced on the head it replaced.
    Created {
        head_id: HeadId,
        silenced_alerts: usize,
    },
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
    use bencher_json::{
        DateTime, HeadIdempotencyKey,
        project::{alert::AlertStatus, boundary::BoundaryLimit},
    };
    use diesel::{
        Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
        SelectableHelper as _, SqliteConnection, connection::SimpleConnection as _,
    };
    use diesel_migrations::MigrationHarness as _;

    use super::{BranchHead, HeadId, InsertHead};
    use crate::{
        model::project::{
            ProjectId,
            branch::{
                BranchId, QueryBranch,
                head_version::QueryHeadVersion,
                start_point::StartPoint,
                version::{QueryVersion, VersionId},
            },
        },
        schema,
        test_util::{
            BranchIds, count_head_versions, create_base_entities, create_branch_with_head,
            create_head_version, create_version, get_branch_head_id, get_head_replaced,
            get_head_versions, setup_test_db,
        },
    };

    const KEY: &str = "00000000-0000-0000-0000-0000000000a1";
    const OTHER_KEY: &str = "00000000-0000-0000-0000-0000000000a2";
    const HEAD_IDEMPOTENCY_KEY_MIGRATION: &str = "20261021120000";

    #[test]
    fn resets_with_one_key_share_one_head() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let start_point = source_start_point(&mut conn, base.project_id);
        let feature = feature_branch(&mut conn, base.project_id);

        let first = created(reset(
            &mut conn,
            feature.branch_id,
            Some(&start_point),
            Some(key(KEY)),
        ));
        let total_head_versions: i64 = schema::head_version::table
            .count()
            .get_result(&mut conn)
            .expect("Failed to count head versions");
        let first_replaced = get_head_replaced(&mut conn, feature.head_id);

        let second = held(reset(
            &mut conn,
            feature.branch_id,
            Some(&start_point),
            Some(key(KEY)),
        ));

        assert_eq!(second, first);
        assert_eq!(
            get_branch_head_id(&mut conn, feature.branch_id),
            Some(first)
        );
        assert_eq!(head_count(&mut conn, feature.branch_id), 2);
        assert_eq!(count_head_versions(&mut conn, first), 3);
        let after: i64 = schema::head_version::table
            .count()
            .get_result(&mut conn)
            .expect("Failed to count head versions");
        assert_eq!(after, total_head_versions, "the reuse clones no versions");
        assert!(get_head_replaced(&mut conn, first).is_none());
        assert_eq!(
            get_head_replaced(&mut conn, feature.head_id),
            first_replaced,
            "the reuse replaces nothing"
        );
    }

    #[test]
    fn a_key_held_by_a_replaced_head_returns_that_head() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let feature = feature_branch(&mut conn, base.project_id);

        let older = created(reset(&mut conn, feature.branch_id, None, Some(key(KEY))));
        let newer = created(reset(
            &mut conn,
            feature.branch_id,
            None,
            Some(key(OTHER_KEY)),
        ));
        assert!(get_head_replaced(&mut conn, older).is_some());

        let late = held(reset(&mut conn, feature.branch_id, None, Some(key(KEY))));

        assert_eq!(late, older);
        assert_eq!(
            get_branch_head_id(&mut conn, feature.branch_id),
            Some(newer)
        );
        assert!(get_head_replaced(&mut conn, newer).is_none());
        assert_eq!(head_count(&mut conn, feature.branch_id), 3);
    }

    #[test]
    fn resets_without_a_key_create_a_head_each_time() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let feature = feature_branch(&mut conn, base.project_id);

        let first = created(reset(&mut conn, feature.branch_id, None, None));
        let second = created(reset(&mut conn, feature.branch_id, None, None));

        assert_ne!(first, second);
        assert_eq!(
            get_branch_head_id(&mut conn, feature.branch_id),
            Some(second)
        );
        assert!(get_head_replaced(&mut conn, feature.head_id).is_some());
        assert!(get_head_replaced(&mut conn, first).is_some());
        assert!(get_head_replaced(&mut conn, second).is_none());
        assert_eq!(head_count(&mut conn, feature.branch_id), 3);
    }

    #[test]
    fn a_key_is_scoped_to_its_branch() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let feature = feature_branch(&mut conn, base.project_id);
        let other = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "other",
            "other",
            "00000000-0000-0000-0000-000000000021",
        );

        let feature_head = created(reset(&mut conn, feature.branch_id, None, Some(key(KEY))));
        let other_head = created(reset(&mut conn, other.branch_id, None, Some(key(KEY))));

        assert_ne!(feature_head, other_head);
        assert_eq!(
            get_branch_head_id(&mut conn, feature.branch_id),
            Some(feature_head)
        );
        assert_eq!(
            get_branch_head_id(&mut conn, other.branch_id),
            Some(other_head)
        );
    }

    #[test]
    fn a_reset_replaces_the_head_current_when_it_writes() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let feature = feature_branch(&mut conn, base.project_id);

        // Both resets read the branch while its first head is current,
        // then an overlapping reset writes first.
        let stale = InsertHead::new(feature.branch_id, None);
        let overlapping = created(reset(&mut conn, feature.branch_id, None, None));
        let last = conn
            .immediate_transaction(|conn| stale.insert_for_branch(conn, None, DateTime::TEST))
            .map(created)
            .expect("Failed to reset the branch");

        assert_eq!(get_branch_head_id(&mut conn, feature.branch_id), Some(last));
        assert!(get_head_replaced(&mut conn, feature.head_id).is_some());
        assert!(
            get_head_replaced(&mut conn, overlapping).is_some(),
            "the head the overlapping reset made is replaced"
        );
        assert!(get_head_replaced(&mut conn, last).is_none());
    }

    // The down migration restores the head table that the migrations before it built,
    // and both ways carry every head.
    #[test]
    fn idempotency_key_migration_down_and_up_keeps_every_head() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let start_point = source_start_point(&mut conn, base.project_id);
        let feature = feature_branch(&mut conn, base.project_id);
        created(reset(
            &mut conn,
            feature.branch_id,
            Some(&start_point),
            Some(key(KEY)),
        ));
        let heads = head_rows(&mut conn);
        let schema = head_schema(&mut conn);

        // Foreign keys are off as in `run_migrations`, so the table rebuild does not cascade.
        conn.batch_execute("PRAGMA foreign_keys = OFF")
            .expect("Failed to disable foreign keys");
        loop {
            let version = conn
                .revert_last_migration(crate::MIGRATIONS)
                .expect("Failed to revert a migration");
            if version.to_string() == HEAD_IDEMPOTENCY_KEY_MIGRATION {
                break;
            }
        }
        assert_eq!(
            head_schema(&mut conn),
            head_schema_before_the_migration(),
            "down"
        );
        assert_eq!(head_rows(&mut conn), heads, "down");

        conn.run_pending_migrations(crate::MIGRATIONS)
            .expect("Failed to apply the migrations");
        conn.batch_execute("PRAGMA foreign_keys = ON")
            .expect("Failed to enable foreign keys");
        assert_eq!(head_schema(&mut conn), schema, "up");
        assert_eq!(head_rows(&mut conn), heads, "up");
    }

    fn key(key: &str) -> HeadIdempotencyKey {
        key.parse().expect("Failed to parse the idempotency key")
    }

    /// Reset the branch as a report would, under one write transaction.
    fn reset(
        conn: &mut SqliteConnection,
        branch_id: BranchId,
        start_point: Option<&StartPoint>,
        idempotency_key: Option<HeadIdempotencyKey>,
    ) -> BranchHead {
        let insert_head = InsertHead::new_with_idempotency_key(
            branch_id,
            start_point.map(StartPoint::head_version_id),
            idempotency_key,
        );
        conn.immediate_transaction(|conn| {
            insert_head.insert_for_branch(conn, start_point, DateTime::TEST)
        })
        .expect("Failed to reset the branch")
    }

    fn created(branch_head: BranchHead) -> HeadId {
        let BranchHead::Created { head_id, .. } = branch_head else {
            panic!("Expected a new head, got {branch_head:?}");
        };
        head_id
    }

    fn held(branch_head: BranchHead) -> HeadId {
        let BranchHead::Held(head_id) = branch_head else {
            panic!("Expected a held head, got {branch_head:?}");
        };
        head_id
    }

    fn head_count(conn: &mut SqliteConnection, branch_id: BranchId) -> i64 {
        schema::head::table
            .filter(schema::head::branch_id.eq(branch_id))
            .count()
            .get_result(conn)
            .expect("Failed to count heads")
    }

    /// A source branch with three versions, as the start point at its newest version.
    fn source_start_point(conn: &mut SqliteConnection, project_id: ProjectId) -> StartPoint {
        let source = create_branch_with_head(
            conn,
            project_id,
            "00000000-0000-0000-0000-000000000030",
            "source",
            "source",
            "00000000-0000-0000-0000-000000000031",
        );
        let mut newest = None;
        for number in 1..=3 {
            let version_id = create_version(
                conn,
                project_id,
                &format!("00000000-0000-0000-0000-0000000003{number:02}"),
                number,
                None,
            );
            create_head_version(conn, source.head_id, version_id);
            newest = Some(version_id);
        }
        let newest = newest.expect("three versions");
        let branch = schema::branch::table
            .filter(schema::branch::id.eq(source.branch_id))
            .select(QueryBranch::as_select())
            .first(conn)
            .expect("Failed to get branch");
        let head_version = schema::head_version::table
            .filter(schema::head_version::head_id.eq(source.head_id))
            .filter(schema::head_version::version_id.eq(newest))
            .select(QueryHeadVersion::as_select())
            .first(conn)
            .expect("Failed to get head version");
        let version = QueryVersion::get(conn, newest).expect("Failed to get version");
        StartPoint {
            branch,
            head_version,
            version,
            max_versions: None,
            clone_thresholds: None,
        }
    }

    #[derive(Debug, PartialEq, Eq, diesel::QueryableByName)]
    struct HeadRow {
        #[diesel(sql_type = diesel::sql_types::Integer)]
        id: i32,
        #[diesel(sql_type = diesel::sql_types::Text)]
        uuid: String,
        #[diesel(sql_type = diesel::sql_types::Integer)]
        branch_id: i32,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Integer>)]
        start_point_id: Option<i32>,
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        created: i64,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::BigInt>)]
        replaced: Option<i64>,
    }

    /// Every head, by the columns the table has with and without the key.
    fn head_rows(conn: &mut SqliteConnection) -> Vec<HeadRow> {
        diesel::sql_query(
            "SELECT id, uuid, branch_id, start_point_id, created, replaced FROM head ORDER BY id",
        )
        .load(conn)
        .expect("Failed to load the heads")
    }

    #[derive(Debug, PartialEq, Eq, diesel::QueryableByName)]
    struct SchemaEntry {
        #[diesel(sql_type = diesel::sql_types::Text)]
        kind: String,
        #[diesel(sql_type = diesel::sql_types::Text)]
        name: String,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        sql: Option<String>,
    }

    /// The head table and its indexes, with whitespace collapsed.
    /// SQLite quotes the name of a table that a rename created, so the quotes are dropped.
    fn head_schema(conn: &mut SqliteConnection) -> Vec<SchemaEntry> {
        diesel::sql_query(
            "SELECT type AS kind, name, sql FROM sqlite_master WHERE tbl_name = 'head' ORDER BY name",
        )
        .load::<SchemaEntry>(conn)
        .expect("Failed to load the head schema")
        .into_iter()
        .map(|SchemaEntry { kind, name, sql }| SchemaEntry {
            kind,
            name,
            sql: sql.map(|sql| {
                sql.split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .replace(r#"CREATE TABLE "head""#, "CREATE TABLE head")
            }),
        })
        .collect()
    }

    fn head_schema_before_the_migration() -> Vec<SchemaEntry> {
        let mut conn =
            SqliteConnection::establish(":memory:").expect("Failed to open the reference database");
        conn.batch_execute("PRAGMA foreign_keys = OFF")
            .expect("Failed to disable foreign keys");
        let pending = conn
            .pending_migrations(crate::MIGRATIONS)
            .expect("Failed to list the migrations");
        for migration in pending.iter().take_while(|migration| {
            migration.name().version().to_string() != HEAD_IDEMPOTENCY_KEY_MIGRATION
        }) {
            conn.run_migration(migration.as_ref())
                .expect("Failed to apply a migration");
        }
        head_schema(&mut conn)
    }

    fn feature_branch(conn: &mut SqliteConnection, project_id: ProjectId) -> BranchIds {
        create_branch_with_head(
            conn,
            project_id,
            "00000000-0000-0000-0000-000000000010",
            "feature",
            "feature",
            "00000000-0000-0000-0000-000000000011",
        )
    }

    /// Test that `head_version` records can be queried by `head_id`.
    /// This is the foundation of the `StartPoint::version_ids` operation.
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
    /// The `StartPoint::version_ids` function uses this filter to clone only versions up to
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
    /// The `StartPoint::version_ids` function uses limit to cap the number of versions cloned.
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

        assert_eq!(version_ids, Vec::new());

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
            .immediate_transaction(|conn| {
                use super::InsertHead;
                use crate::macros::sql::last_insert_rowid;

                let insert_head = InsertHead::new(branch.branch_id, None);
                diesel::insert_into(schema::head::table)
                    .values(&insert_head)
                    .execute(conn)?;
                let new_head_id: HeadId = diesel::select(last_insert_rowid()).get_result(conn)?;

                diesel::update(
                    schema::branch::table.filter(schema::branch::id.eq(branch.branch_id)),
                )
                .set(schema::branch::head_id.eq(new_head_id))
                .execute(conn)?;

                diesel::QueryResult::Ok(new_head_id)
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
        conn.immediate_transaction(|conn| {
            use super::{InsertHead, UpdateHead};
            use crate::macros::sql::last_insert_rowid;

            let insert_head = InsertHead::new(branch.branch_id, None);
            diesel::insert_into(schema::head::table)
                .values(&insert_head)
                .execute(conn)?;
            let new_head_id: HeadId = diesel::select(last_insert_rowid()).get_result(conn)?;

            diesel::update(schema::branch::table.filter(schema::branch::id.eq(branch.branch_id)))
                .set(schema::branch::head_id.eq(new_head_id))
                .execute(conn)?;

            // Mark old head as replaced
            let update_head = UpdateHead::replace();
            diesel::update(schema::head::table.filter(schema::head::id.eq(branch.head_id)))
                .set(&update_head)
                .execute(conn)?;

            diesel::QueryResult::Ok(())
        })
        .expect("Transaction failed");

        // Old head should now be replaced
        assert!(get_head_replaced(&mut conn, branch.head_id).is_some());
    }

    /// Test that silencing alerts for old head works within a transaction.
    #[test]
    #[expect(clippy::too_many_lines, reason = "test requires extensive setup")]
    fn for_branch_silences_old_head_alerts() {
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
            BoundaryLimit::Upper,
            AlertStatus::Active,
        );

        // Alert should be active
        assert_eq!(
            crate::test_util::get_alert_status(&mut conn, alert_id),
            AlertStatus::Active,
        );

        // Run transaction: silence alerts for old head
        conn.immediate_transaction(|conn| {
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
                let silenced_alert = UpdateAlert::silence(DateTime::TEST);
                diesel::update(schema::alert::table.filter(schema::alert::id.eq_any(&alerts)))
                    .set(&silenced_alert)
                    .execute(conn)?;
            }

            diesel::QueryResult::Ok(())
        })
        .expect("Transaction failed");

        // Alert should now be silenced
        assert_eq!(
            crate::test_util::get_alert_status(&mut conn, alert_id),
            AlertStatus::Silenced,
        );
    }

    /// Test that transaction works correctly when branch has no old head.
    #[test]
    fn for_branch_no_old_head_skips_replace() {
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

        let branch_id: BranchId = {
            use crate::macros::sql::last_insert_rowid;
            diesel::select(last_insert_rowid())
                .get_result(&mut conn)
                .expect("Failed to get branch id")
        };

        // Branch has no head
        assert!(get_branch_head_id(&mut conn, branch_id).is_none());

        // Run transaction: insert new head, update branch, no old head to replace
        let new_head_id = conn
            .immediate_transaction(|conn| {
                use super::InsertHead;
                use crate::macros::sql::last_insert_rowid;

                let insert_head = InsertHead::new(branch_id, None);
                diesel::insert_into(schema::head::table)
                    .values(&insert_head)
                    .execute(conn)?;
                let new_head_id: HeadId = diesel::select(last_insert_rowid()).get_result(conn)?;

                diesel::update(schema::branch::table.filter(schema::branch::id.eq(branch_id)))
                    .set(schema::branch::head_id.eq(new_head_id))
                    .execute(conn)?;

                diesel::QueryResult::Ok(new_head_id)
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
