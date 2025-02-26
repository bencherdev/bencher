use bencher_json::{
    project::head::{JsonVersion, VersionNumber},
    GitHash, VersionUuid,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::resource_conflict_err,
    macros::fn_get::{fn_get, fn_get_id, fn_get_uuid},
    schema,
    schema::version as version_table,
};

use super::{head::HeadId, head_version::InsertHeadVersion, ProjectId, QueryProject};

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

    pub fn get_or_increment(
        conn: &mut DbConnection,
        project_id: ProjectId,
        head_id: HeadId,
        hash: Option<&GitHash>,
    ) -> Result<VersionId, HttpError> {
        if let Some(hash) = hash {
            // We need to join directly back to the report.
            // This ensures that we are only looking for code versions for the current branch head that generated the report.
            // That is, we do not want to use the exact same code version for a branch head that was later used us as a start point.
            if let Ok(version_id) = schema::version::table
                .inner_join(schema::report::table)
                .filter(schema::report::head_id.eq(head_id))
                .filter(schema::version::project_id.eq(project_id))
                .filter(schema::version::hash.eq(hash.as_ref()))
                .order(schema::version::number.desc())
                .select(schema::version::id)
                .first::<VersionId>(conn)
            {
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
    pub fn increment(
        conn: &mut DbConnection,
        project_id: ProjectId,
        head_id: HeadId,
        hash: Option<GitHash>,
    ) -> Result<VersionId, HttpError> {
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
            hash: hash.map(Into::into),
        };

        diesel::insert_into(schema::version::table)
            .values(&insert_version)
            .execute(conn)
            .map_err(resource_conflict_err!(Version, insert_version))?;

        let version_id = QueryVersion::get_id(conn, version_uuid)?;

        let insert_head_version = InsertHeadVersion {
            head_id,
            version_id,
        };

        diesel::insert_into(schema::head_version::table)
            .values(&insert_head_version)
            .execute(conn)
            .map_err(resource_conflict_err!(HeadVersion, insert_head_version))?;

        Ok(version_id)
    }
}
