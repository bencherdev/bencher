use bencher_json::{
    GitHash, JsonNewStartPoint,
    project::{
        branch::{JsonUpdateStartPoint, START_POINT_MAX_VERSIONS},
        head::VersionNumber,
    },
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::is_not_found,
    model::project::ProjectId,
    schema,
};

use super::{
    QueryBranch,
    head_version::{HeadVersionId, QueryHeadVersion},
    version::{QueryVersion, VersionId},
};

#[derive(Debug, Clone)]
pub struct StartPoint {
    pub branch: QueryBranch,
    pub head_version: QueryHeadVersion,
    pub version: QueryVersion,
    pub max_versions: Option<u32>,
    pub clone_thresholds: Option<bool>,
}

impl StartPoint {
    pub async fn new(
        context: &ApiContext,
        query_branch: QueryBranch,
        head_version: QueryHeadVersion,
        max_versions: Option<u32>,
        clone_thresholds: Option<bool>,
    ) -> Result<Self, HttpError> {
        let version = QueryVersion::get(auth_conn!(context), head_version.version_id)?;
        Ok(Self {
            branch: query_branch,
            head_version,
            version,
            max_versions,
            clone_thresholds,
        })
    }

    pub async fn latest_for_branch(
        context: &ApiContext,
        project_id: ProjectId,
        query_branch: QueryBranch,
        hash: Option<&GitHash>,
        max_versions: Option<u32>,
        clone_thresholds: Option<bool>,
    ) -> Result<Self, HttpError> {
        // If a hash is specified but not found, fall back to the latest version without the hash.
        // https://github.com/bencherdev/bencher/issues/774
        let head_version =
            match QueryHeadVersion::get_latest_for_branch(context, project_id, &query_branch, hash)
                .await
            {
                Ok(hv) => hv,
                Err(err) if hash.is_some() && is_not_found(&err) => {
                    QueryHeadVersion::get_latest_for_branch(
                        context,
                        project_id,
                        &query_branch,
                        None,
                    )
                    .await?
                },
                Err(err) => return Err(err),
            };
        Self::new(
            context,
            query_branch,
            head_version,
            max_versions,
            clone_thresholds,
        )
        .await
    }

    pub async fn from_new_json(
        context: &ApiContext,
        project_id: ProjectId,
        json: JsonNewStartPoint,
    ) -> Result<Self, HttpError> {
        let JsonNewStartPoint {
            branch,
            hash,
            max_versions,
            clone_thresholds,
        } = json;
        let query_branch = QueryBranch::from_name_id(auth_conn!(context), project_id, &branch)?;
        Self::latest_for_branch(
            context,
            project_id,
            query_branch,
            hash.as_ref(),
            max_versions,
            clone_thresholds,
        )
        .await
    }

    pub async fn from_update_json(
        context: &ApiContext,
        project_id: ProjectId,
        json: Option<&JsonUpdateStartPoint>,
    ) -> Result<Option<Self>, HttpError> {
        // Get the new start point, if there is a branch specified.
        let Some(JsonUpdateStartPoint {
            branch: Some(branch),
            hash,
            max_versions,
            clone_thresholds,
            reset: _,
        }) = json
        else {
            return Ok(None);
        };
        // If updating the start point, it is okay if it does not exist.
        // This avoids a race condition when creating both the branch and start point in CI.
        let query_branch = match QueryBranch::from_name_id(auth_conn!(context), project_id, branch)
        {
            Ok(query_branch) => query_branch,
            Err(err) if is_not_found(&err) => return Ok(None),
            Err(err) => return Err(err),
        };
        // If updating the start point, it is okay if it does not exist.
        // https://github.com/bencherdev/bencher/issues/774
        match Self::latest_for_branch(
            context,
            project_id,
            query_branch,
            hash.as_ref(),
            *max_versions,
            *clone_thresholds,
        )
        .await
        {
            Ok(start_point) => Ok(Some(start_point)),
            Err(err) if is_not_found(&err) => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub fn head_version_id(&self) -> HeadVersionId {
        self.head_version.id
    }

    pub fn max_versions(&self) -> u32 {
        self.max_versions.unwrap_or(START_POINT_MAX_VERSIONS)
    }

    pub fn version_ids(&self, conn: &mut DbConnection) -> diesel::QueryResult<Vec<VersionId>> {
        let number: VersionNumber = schema::version::table
            .filter(schema::version::id.eq(self.head_version.version_id))
            .select(schema::version::number)
            .first(conn)?;
        schema::head_version::table
            .inner_join(schema::version::table)
            .filter(schema::head_version::head_id.eq(self.head_version.head_id))
            .filter(schema::version::number.le(number))
            .order(schema::version::number.desc())
            .limit(i64::from(self.max_versions()))
            .select(schema::head_version::version_id)
            .load(conn)
    }
}

#[cfg(test)]
mod tests {
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _};

    use crate::{
        model::project::branch::{
            QueryBranch,
            head_version::QueryHeadVersion,
            version::{QueryVersion, VersionId},
        },
        schema,
        test_util::{
            create_base_entities, create_branch_with_head, create_head_version, create_version,
            setup_test_db,
        },
    };

    use super::StartPoint;

    /// A start point resolved before a renumber must not copy a version that
    /// took the start point's old number on a newer id.
    #[test]
    fn version_ids_reads_the_start_point_number_at_copy_time() {
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
        let versions: Vec<VersionId> = (1..=5)
            .map(|i| {
                let v = create_version(
                    &mut conn,
                    base.project_id,
                    &format!("00000000-0000-0000-0000-0000000001{i:02}"),
                    i,
                    None,
                );
                create_head_version(&mut conn, source.head_id, v);
                v
            })
            .collect();
        let [v1, v2, v3, v4, v5]: [VersionId; 5] = versions.try_into().expect("five versions");

        // Resolve the start point at the newest version, number 5.
        let branch = schema::branch::table
            .filter(schema::branch::id.eq(source.branch_id))
            .select(QueryBranch::as_select())
            .first(&mut conn)
            .expect("Failed to get branch");
        let head_version = schema::head_version::table
            .filter(schema::head_version::head_id.eq(source.head_id))
            .filter(schema::head_version::version_id.eq(v5))
            .select(QueryHeadVersion::as_select())
            .first(&mut conn)
            .expect("Failed to get head version");
        let version = QueryVersion::get(&mut conn, v5).expect("Failed to get version");
        let start_point = StartPoint {
            branch,
            head_version,
            version,
            max_versions: None,
            clone_thresholds: None,
        };

        // A report delete removes version 2 and renumbers 3 to 5 down to 2 to 4.
        diesel::delete(schema::version::table.filter(schema::version::id.eq(v2)))
            .execute(&mut conn)
            .expect("Failed to delete version");
        for (v, n) in [(v3, 2), (v4, 3), (v5, 4)] {
            diesel::update(schema::version::table.filter(schema::version::id.eq(v)))
                .set(schema::version::number.eq(n))
                .execute(&mut conn)
                .expect("Failed to renumber version");
        }
        // An upload then takes the freed number 5 on a newer id.
        let newer = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000199",
            5,
            None,
        );
        create_head_version(&mut conn, source.head_id, newer);

        let version_ids = start_point
            .version_ids(&mut conn)
            .expect("Failed to load version ids");
        assert_eq!(
            version_ids,
            vec![v5, v4, v3, v1],
            "only versions at or below the start point's current number, newest first"
        );
        assert!(
            !version_ids.contains(&newer),
            "the newer version must not be copied"
        );
    }
}
