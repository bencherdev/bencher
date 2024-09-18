use bencher_json::{
    project::reference::{JsonVersion, VersionNumber},
    GitHash, VersionUuid,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::resource_conflict_err,
    schema,
    schema::version as version_table,
    util::fn_get::{fn_get, fn_get_id, fn_get_uuid},
};

use super::{
    reference::ReferenceId, reference_version::InsertReferenceVersion, ProjectId, QueryProject,
};

crate::util::typed_id::typed_id!(VersionId);

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
        reference_id: ReferenceId,
        hash: Option<&GitHash>,
    ) -> Result<VersionId, HttpError> {
        if let Some(hash) = hash {
            if let Ok(version_id) = schema::version::table
                .inner_join(schema::reference_version::table)
                .filter(schema::reference_version::reference_id.eq(reference_id))
                .filter(schema::version::project_id.eq(project_id))
                .filter(schema::version::hash.eq(hash.as_ref()))
                .order(schema::version::number.desc())
                .select(schema::version::id)
                .first::<VersionId>(conn)
            {
                Ok(version_id)
            } else {
                InsertVersion::increment(conn, project_id, reference_id, Some(hash.clone()))
            }
        } else {
            InsertVersion::increment(conn, project_id, reference_id, None)
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
        reference_id: ReferenceId,
        hash: Option<GitHash>,
    ) -> Result<VersionId, HttpError> {
        // Get the most recent code version number for this branch and increment it.
        // Otherwise, start a new branch code version number count from zero.
        let number = if let Ok(number) = schema::version::table
            .inner_join(schema::reference_version::table)
            .filter(schema::reference_version::reference_id.eq(reference_id))
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

        let insert_reference_version = InsertReferenceVersion {
            reference_id,
            version_id,
        };

        diesel::insert_into(schema::reference_version::table)
            .values(&insert_reference_version)
            .execute(conn)
            .map_err(resource_conflict_err!(
                ReferenceVersion,
                insert_reference_version
            ))?;

        Ok(version_id)
    }
}
