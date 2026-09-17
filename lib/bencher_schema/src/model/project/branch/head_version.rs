use bencher_json::{BranchUuid, GitHash, HeadUuid, JsonStartPoint};
use diesel::{
    ExpressionMethods as _, JoinOnDsl as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _,
};
use dropshot::HttpError;

use crate::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::resource_not_found_err,
    macros::fn_get::fn_get,
    schema::{self, head_version as head_version_table},
};

use super::{
    ProjectId, QueryBranch,
    head::HeadId,
    version::{QueryVersion, VersionId},
};

crate::macros::typed_id::typed_id!(HeadVersionId);

#[derive(Debug, Clone, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = head_version_table)]
pub struct QueryHeadVersion {
    pub id: HeadVersionId,
    pub head_id: HeadId,
    pub version_id: VersionId,
}

impl QueryHeadVersion {
    fn_get!(head_version, HeadVersionId);

    pub async fn get_latest_for_branch(
        context: &ApiContext,
        project_id: ProjectId,
        query_branch: &QueryBranch,
        hash: Option<&GitHash>,
    ) -> Result<Self, HttpError> {
        let head_id = query_branch.head_id()?;
        let mut query = schema::head_version::table
            .inner_join(schema::version::table)
            // Filter for the branch head
            .filter(schema::head_version::head_id.eq(head_id))
            // Sanity check that we are in the right project
            .filter(schema::version::project_id.eq(project_id))
            .into_boxed();

        if let Some(hash) = hash {
            // Make sure the start point version has the correct hash, if specified.
            query = query.filter(schema::version::hash.eq(hash));
        }

        query
            // If the hash is not specified, get the most recent version.
            .order(schema::version::number.desc())
            .select(Self::as_select())
            .first::<Self>(auth_conn!(context))
            .map_err(resource_not_found_err!(
                HeadVersion,
                (query_branch, hash)
            ))
    }

    pub fn into_start_point_json(
        self,
        conn: &mut DbConnection,
    ) -> Result<JsonStartPoint, HttpError> {
        let (branch, head) = schema::branch::table
            .inner_join(schema::head::table.on(schema::head::branch_id.eq(schema::branch::id)))
            .filter(schema::head::id.eq(self.head_id))
            .select((schema::branch::uuid, schema::head::uuid))
            .first::<(BranchUuid, HeadUuid)>(conn)
            .map_err(resource_not_found_err!(Head, self.head_id))?;
        let version = QueryVersion::get(conn, self.version_id)?.into_json();
        Ok(JsonStartPoint {
            branch,
            head,
            version,
        })
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = head_version_table)]
pub struct InsertHeadVersion {
    pub head_id: HeadId,
    pub version_id: VersionId,
}

impl InsertHeadVersion {
    pub fn insert_all(
        conn: &mut DbConnection,
        head_id: HeadId,
        version_ids: &[VersionId],
    ) -> diesel::QueryResult<()> {
        if version_ids.is_empty() {
            return Ok(());
        }
        let insert_head_versions: Vec<Self> = version_ids
            .iter()
            .map(|&version_id| Self {
                head_id,
                version_id,
            })
            .collect();
        diesel::insert_into(schema::head_version::table)
            .values(&insert_head_versions)
            .execute(conn)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::{GitHash, project::head::VersionNumber};
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _};

    use crate::{
        context::DbConnection,
        model::project::{
            ProjectId,
            branch::{
                QueryBranch,
                head::HeadId,
                start_point::StartPoint,
                version::{InsertVersion, QueryVersion, VersionId},
            },
        },
        schema,
        test_util::{create_base_entities, create_branch_with_head, setup_test_db},
    };

    use super::{InsertHeadVersion, QueryHeadVersion};

    fn increment(
        conn: &mut DbConnection,
        project_id: ProjectId,
        head_id: HeadId,
        hash: Option<GitHash>,
    ) -> VersionId {
        conn.immediate_transaction(|conn| InsertVersion::increment(conn, project_id, head_id, hash))
            .expect("Failed to increment version")
    }

    fn head_history(conn: &mut DbConnection, head_id: HeadId) -> Vec<(VersionId, VersionNumber)> {
        schema::head_version::table
            .inner_join(schema::version::table)
            .filter(schema::head_version::head_id.eq(head_id))
            .order(schema::head_version::version_id.asc())
            .select((schema::head_version::version_id, schema::version::number))
            .load(conn)
            .expect("Failed to load head history")
    }

    /// Both write paths that add head versions, the increment and the start
    /// point copy, keep a head's version numbers rising with its version ids.
    #[test]
    fn head_versions_number_order_follows_version_id_order() {
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
        let hash: GitHash = "1234567890abcdef1234567890abcdef12345678"
            .parse()
            .expect("valid hash");

        let [s0, s1, s2, s3]: [VersionId; 4] = [Some(hash.clone()), None, Some(hash.clone()), None]
            .map(|hash| increment(&mut conn, base.project_id, source.head_id, hash));

        let branch = schema::branch::table
            .filter(schema::branch::id.eq(source.branch_id))
            .select(QueryBranch::as_select())
            .first(&mut conn)
            .expect("Failed to get branch");
        let head_version = schema::head_version::table
            .filter(schema::head_version::head_id.eq(source.head_id))
            .filter(schema::head_version::version_id.eq(s2))
            .select(QueryHeadVersion::as_select())
            .first(&mut conn)
            .expect("Failed to get head version");
        let version = QueryVersion::get(&mut conn, s2).expect("Failed to get version");
        let start_point = StartPoint {
            branch,
            head_version,
            version,
            max_versions: Some(2),
            clone_thresholds: None,
        };
        let version_ids = start_point
            .version_ids(&mut conn)
            .expect("Failed to load version ids");
        assert_eq!(version_ids, vec![s2, s1], "copy block, newest first");
        InsertHeadVersion::insert_all(&mut conn, dest.head_id, &version_ids)
            .expect("Failed to copy head versions");

        let d3 = increment(&mut conn, base.project_id, dest.head_id, Some(hash));
        let d4 = increment(&mut conn, base.project_id, dest.head_id, None);
        let s4 = increment(&mut conn, base.project_id, source.head_id, None);

        let source_history = head_history(&mut conn, source.head_id);
        let dest_history = head_history(&mut conn, dest.head_id);
        assert_eq!(
            source_history,
            vec![
                (s0, VersionNumber(0)),
                (s1, VersionNumber(1)),
                (s2, VersionNumber(2)),
                (s3, VersionNumber(3)),
                (s4, VersionNumber(4)),
            ],
            "source head in version id order"
        );
        assert_eq!(
            dest_history,
            vec![
                (s1, VersionNumber(1)),
                (s2, VersionNumber(2)),
                (d3, VersionNumber(3)),
                (d4, VersionNumber(4)),
            ],
            "new head in version id order"
        );
        assert!(
            d3 > s3,
            "versions added after the copy take ids above every source id at copy time"
        );
        for history in [source_history, dest_history] {
            assert!(
                history.is_sorted_by(|a, b| a.1.0 < b.1.0),
                "version numbers rise strictly with version ids"
            );
        }
    }
}
