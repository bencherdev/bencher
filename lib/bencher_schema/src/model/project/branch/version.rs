use bencher_json::{
    GitHash, VersionUuid,
    project::head::{JsonVersion, VersionNumber},
};
use diesel::{
    ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _,
    result::QueryResult,
};

use crate::{
    context::DbConnection,
    macros::{
        fn_get::{fn_get, fn_get_id, fn_get_uuid},
        sql::last_insert_rowid,
    },
    schema,
    schema::version as version_table,
};

use super::{ProjectId, QueryProject, head::HeadId, head_version::InsertHeadVersion};

crate::macros::typed_id::typed_id!(VersionId);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = version_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryVersion {
    pub id: VersionId,
    pub uuid: VersionUuid,
    pub project_id: ProjectId,
    pub number: VersionNumber,
    pub hash: Option<GitHash>,
}

impl QueryVersion {
    fn_get!(version, VersionId);
    fn_get_id!(version, VersionId, VersionUuid);
    fn_get_uuid!(version, VersionId, VersionUuid);

    /// Read-only lookup: find an existing version by git hash for the given branch head.
    /// Used outside the write transaction to avoid holding the write lock for read-only queries.
    pub fn find_by_hash(
        conn: &mut DbConnection,
        project_id: ProjectId,
        head_id: HeadId,
        hash: &GitHash,
    ) -> QueryResult<Option<VersionId>> {
        schema::version::table
            .inner_join(schema::report::table)
            .filter(schema::report::head_id.eq(head_id))
            .filter(schema::version::project_id.eq(project_id))
            .filter(schema::version::hash.eq(hash.as_ref()))
            .order(schema::version::number.desc())
            .select(schema::version::id)
            .first::<VersionId>(conn)
            .optional()
    }

    pub fn get_or_increment(
        conn: &mut DbConnection,
        project_id: ProjectId,
        head_id: HeadId,
        hash: Option<&GitHash>,
    ) -> QueryResult<VersionId> {
        if let Some(hash) = hash {
            if let Some(version_id) = Self::find_by_hash(conn, project_id, head_id, hash)? {
                Ok(version_id)
            } else {
                InsertVersion::increment(conn, project_id, head_id, Some(hash.clone()))
            }
        } else {
            InsertVersion::increment(conn, project_id, head_id, None)
        }
    }

    pub fn into_json(self) -> JsonVersion {
        let Self { number, hash, .. } = self;
        JsonVersion { number, hash }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = version_table)]
pub struct InsertVersion {
    pub uuid: VersionUuid,
    pub project_id: ProjectId,
    pub number: VersionNumber,
    pub hash: Option<GitHash>,
}

impl InsertVersion {
    fn increment(
        conn: &mut DbConnection,
        project_id: ProjectId,
        head_id: HeadId,
        hash: Option<GitHash>,
    ) -> QueryResult<VersionId> {
        // Get the most recent code version number for this branch head and increment it.
        // Otherwise, start a new branch code version number count from zero.
        // Do NOT join directly to the report for the particular branch head.
        // This ensures that we continue to increment properly for a branch head that used us as a start point.
        let number = if let Ok(number) = schema::version::table
            .inner_join(schema::head_version::table)
            .filter(schema::head_version::head_id.eq(head_id))
            .select(schema::version::number)
            .order(schema::version::number.desc())
            .first::<VersionNumber>(conn)
        {
            number.increment()
        } else {
            VersionNumber::default()
        };

        let version_uuid = VersionUuid::new();
        let insert_version = InsertVersion {
            uuid: version_uuid,
            project_id,
            number,
            hash,
        };

        diesel::insert_into(schema::version::table)
            .values(&insert_version)
            .execute(conn)?;

        let version_id = diesel::select(last_insert_rowid()).get_result::<VersionId>(conn)?;

        let insert_head_version = InsertHeadVersion {
            head_id,
            version_id,
        };

        diesel::insert_into(schema::head_version::table)
            .values(&insert_head_version)
            .execute(conn)?;

        Ok(version_id)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        model::project::{ProjectId, branch::head::HeadId},
        test_util::{
            create_base_entities, create_branch_with_head, create_head_version, create_report,
            create_testbed, create_version, setup_test_db,
        },
    };

    use super::{QueryVersion, VersionId};

    #[test]
    fn find_by_hash_returns_existing_version() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000020",
        );
        let testbed_id = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "localhost",
            "localhost",
        );
        let version_id = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000040",
            0,
            Some("1234567890abcdef1234567890abcdef12345678"),
        );
        create_head_version(&mut conn, branch.head_id, version_id);

        // Create a report that references this head + version
        create_report(
            &mut conn,
            "00000000-0000-0000-0000-000000000050",
            base.project_id,
            branch.head_id,
            version_id,
            testbed_id,
        );

        let hash: bencher_json::GitHash = "1234567890abcdef1234567890abcdef12345678"
            .parse()
            .expect("valid hash");
        let result = QueryVersion::find_by_hash(
            &mut conn,
            ProjectId::from_raw(base.project_id),
            HeadId::from_raw(branch.head_id),
            &hash,
        );
        assert_eq!(result.unwrap(), Some(VersionId::from_raw(version_id)));
    }

    #[test]
    fn find_by_hash_returns_none_for_missing_hash() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000020",
        );

        let hash: bencher_json::GitHash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .parse()
            .expect("valid hash");
        let result = QueryVersion::find_by_hash(
            &mut conn,
            ProjectId::from_raw(base.project_id),
            HeadId::from_raw(branch.head_id),
            &hash,
        );
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn find_by_hash_isolates_by_head() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let branch_a = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "branch-a",
            "branch-a",
            "00000000-0000-0000-0000-000000000020",
        );
        let branch_b = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000011",
            "branch-b",
            "branch-b",
            "00000000-0000-0000-0000-000000000021",
        );
        let testbed_id = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "localhost",
            "localhost",
        );
        let version_id = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000040",
            0,
            Some("1234567890abcdef1234567890abcdef12345678"),
        );
        create_head_version(&mut conn, branch_a.head_id, version_id);

        // Report is on branch_a's head
        create_report(
            &mut conn,
            "00000000-0000-0000-0000-000000000050",
            base.project_id,
            branch_a.head_id,
            version_id,
            testbed_id,
        );

        let hash: bencher_json::GitHash = "1234567890abcdef1234567890abcdef12345678"
            .parse()
            .expect("valid hash");

        // Should find on branch_a's head
        let result = QueryVersion::find_by_hash(
            &mut conn,
            ProjectId::from_raw(base.project_id),
            HeadId::from_raw(branch_a.head_id),
            &hash,
        );
        assert_eq!(result.unwrap(), Some(VersionId::from_raw(version_id)));

        // Should NOT find on branch_b's head (no report there)
        let result = QueryVersion::find_by_hash(
            &mut conn,
            ProjectId::from_raw(base.project_id),
            HeadId::from_raw(branch_b.head_id),
            &hash,
        );
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn find_by_hash_returns_latest_version_number() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000020",
        );
        let testbed_id = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "localhost",
            "localhost",
        );

        // Create two versions with the same hash but different version numbers
        let version_old = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000040",
            0,
            Some("1234567890abcdef1234567890abcdef12345678"),
        );
        create_head_version(&mut conn, branch.head_id, version_old);
        create_report(
            &mut conn,
            "00000000-0000-0000-0000-000000000050",
            base.project_id,
            branch.head_id,
            version_old,
            testbed_id,
        );

        let version_new = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000041",
            1,
            Some("1234567890abcdef1234567890abcdef12345678"),
        );
        create_head_version(&mut conn, branch.head_id, version_new);
        create_report(
            &mut conn,
            "00000000-0000-0000-0000-000000000051",
            base.project_id,
            branch.head_id,
            version_new,
            testbed_id,
        );

        let hash: bencher_json::GitHash = "1234567890abcdef1234567890abcdef12345678"
            .parse()
            .expect("valid hash");
        let result = QueryVersion::find_by_hash(
            &mut conn,
            ProjectId::from_raw(base.project_id),
            HeadId::from_raw(branch.head_id),
            &hash,
        );
        // Should return the version with the highest number (most recent)
        assert_eq!(result.unwrap(), Some(VersionId::from_raw(version_new)));
    }
}
