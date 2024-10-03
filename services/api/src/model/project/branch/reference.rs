use bencher_json::{
    project::reference::{JsonVersion, VersionNumber},
    BranchUuid, DateTime, GitHash, JsonHead, JsonStartPoint, ReferenceUuid,
};
use diesel::{
    ExpressionMethods, JoinOnDsl, NullableExpressionMethods, QueryDsl, RunQueryDsl,
    SelectableHelper,
};

use dropshot::HttpError;
use http::StatusCode;
use slog::Logger;

use super::{
    reference_version::{InsertReferenceVersion, ReferenceVersionId},
    start_point::StartPoint,
    version::{QueryVersion, VersionId},
    BranchId, QueryBranch,
};
use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{issue_error, resource_conflict_err, resource_not_found_err},
    model::project::{
        threshold::{alert::QueryAlert, InsertThreshold},
        ProjectId,
    },
    schema::{self, reference as reference_table},
    util::fn_get::fn_get,
};

crate::util::typed_id::typed_id!(ReferenceId);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = reference_table)]
#[diesel(belongs_to(QueryBranch, foreign_key = branch_id))]
pub struct QueryReference {
    pub id: ReferenceId,
    pub uuid: ReferenceUuid,
    pub branch_id: BranchId,
    pub start_point_id: Option<ReferenceVersionId>,
    pub created: DateTime,
    pub replaced: Option<DateTime>,
}

impl QueryReference {
    fn_get!(reference, ReferenceId);

    pub fn from_uuid(
        conn: &mut DbConnection,
        project_id: ProjectId,
        reference_uuid: ReferenceUuid,
    ) -> Result<Self, HttpError> {
        schema::reference::table
            .inner_join(
                schema::branch::table.on(schema::branch::id.eq(schema::reference::branch_id)),
            )
            .filter(schema::branch::project_id.eq(project_id))
            .filter(schema::reference::uuid.eq(reference_uuid))
            .select(Self::as_select())
            .first(conn)
            .map_err(resource_not_found_err!(
                Reference,
                (project_id, reference_uuid)
            ))
    }

    pub fn get_head_json(
        conn: &mut DbConnection,
        reference_id: ReferenceId,
        version: Option<QueryVersion>,
    ) -> Result<JsonHead, HttpError> {
        let (reference_uuid, start_point_id, created, replaced) = schema::reference::table
            .inner_join(
                schema::branch::table.on(schema::branch::id.eq(schema::reference::branch_id)),
            )
            .filter(schema::reference::id.eq(reference_id))
            .select((
                schema::reference::uuid,
                schema::reference::start_point_id.nullable(),
                schema::reference::created,
                schema::reference::replaced.nullable(),
            ))
            .first::<(
                ReferenceUuid,
                Option<ReferenceVersionId>,
                DateTime,
                Option<DateTime>,
            )>(conn)
            .map_err(resource_not_found_err!(Reference, reference_id))?;

        let start_point = if let Some(start_point_id) = start_point_id {
            let (branch, head, number, hash) = schema::reference_version::table
                .inner_join(
                    schema::reference::table
                        .on(schema::reference::id.eq(schema::reference_version::reference_id))
                        .inner_join(
                            schema::branch::table
                                .on(schema::branch::id.eq(schema::reference::branch_id)),
                        ),
                )
                .inner_join(schema::version::table)
                .filter(schema::reference_version::id.eq(start_point_id))
                .select((
                    schema::branch::uuid,
                    schema::reference::uuid,
                    schema::version::number,
                    schema::version::hash.nullable(),
                ))
                .first::<(BranchUuid, ReferenceUuid, VersionNumber, Option<GitHash>)>(conn)
                .map_err(resource_not_found_err!(ReferenceVersion, start_point_id))?;

            Some(JsonStartPoint {
                branch,
                head,
                version: JsonVersion { number, hash },
            })
        } else {
            None
        };

        Ok(JsonHead {
            uuid: reference_uuid,
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
                    start_point_id, branch_start_point.reference_version.id,
                    "Branch start point mismatch"
                );
                self.clone_versions(log, context, branch_start_point)
                    .await?;
                InsertThreshold::from_start_point(log, context, query_branch, branch_start_point)
                    .await
            },
            (None, None) => Ok(()),
            _ => Err(issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Branch start point mismatch",
                "Failed to match branch start point for reference",
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
            branch_start_point.reference_version.version_id,
        )?;
        slog::debug!(log, "Got start point version: {start_point_version:?}");

        // Get all prior versions (version number less than or equal to) for the start point reference
        let version_ids = schema::reference_version::table
            .inner_join(schema::version::table)
            .filter(
                schema::reference_version::reference_id
                    .eq(branch_start_point.reference_version.reference_id),
            )
            .filter(schema::version::number.le(start_point_version.number))
            .order(schema::version::number.desc())
            .limit(i64::from(branch_start_point.max_versions()))
            .select(schema::reference_version::version_id)
            .load::<VersionId>(conn_lock!(context))
            .map_err(resource_not_found_err!(
                ReferenceVersion,
                (branch_start_point, start_point_version)
            ))?;
        slog::debug!(log, "Got version ids: {version_ids:?}");

        // Add new reference to all start point reference versions
        for version_id in version_ids {
            let insert_reference_version = InsertReferenceVersion {
                reference_id: self.id,
                version_id,
            };
            diesel::insert_into(schema::reference_version::table)
                .values(&insert_reference_version)
                .execute(conn_lock!(context))
                .map_err(resource_conflict_err!(
                    ReferenceVersion,
                    insert_reference_version
                ))?;
            slog::debug!(
                log,
                "Inserted reference version: {insert_reference_version:?}"
            );
        }

        slog::debug!(log, "Cloned all reference versions");
        Ok(())
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = reference_table)]
pub struct InsertReference {
    pub uuid: ReferenceUuid,
    pub branch_id: BranchId,
    pub start_point_id: Option<ReferenceVersionId>,
    pub created: DateTime,
    pub replaced: Option<DateTime>,
}

pub struct CloneThresholds {
    pub old_branch_id: BranchId,
    pub new_branch_id: BranchId,
}

impl InsertReference {
    pub fn new(branch_id: BranchId, start_point_id: Option<ReferenceVersionId>) -> Self {
        Self {
            uuid: ReferenceUuid::new(),
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
    ) -> Result<(QueryBranch, QueryReference), HttpError> {
        // Create the head reference for the branch
        let insert_reference = Self::new(
            query_branch.id,
            branch_start_point.map(StartPoint::reference_version_id),
        );
        diesel::insert_into(schema::reference::table)
            .values(&insert_reference)
            .execute(conn_lock!(context))
            .map_err(resource_conflict_err!(Reference, insert_reference))?;
        slog::debug!(log, "Created reference: {insert_reference:?}");

        // Get the new reference
        let query_reference = schema::reference::table
            .filter(schema::reference::uuid.eq(&insert_reference.uuid))
            .first::<QueryReference>(conn_lock!(context))
            .map_err(resource_not_found_err!(Reference, insert_reference))?;
        slog::debug!(log, "Got reference: {query_reference:?}");

        // Update the branch head reference
        diesel::update(schema::branch::table.filter(schema::branch::id.eq(query_branch.id)))
            .set(schema::branch::head_id.eq(query_reference.id))
            .execute(conn_lock!(context))
            .map_err(resource_conflict_err!(
                Branch,
                (&query_branch, &query_reference)
            ))?;
        slog::debug!(log, "Updated branch: {query_branch:?}");

        // If the branch has an old head reference, then mark it as replaced.
        // This should not run if the branch is new.
        if let Some(old_head_id) = query_branch.head_id {
            let update_reference = UpdateReference::replace();
            diesel::update(schema::reference::table.filter(schema::reference::id.eq(old_head_id)))
                .set(&update_reference)
                .execute(conn_lock!(context))
                .map_err(resource_conflict_err!(
                    Reference,
                    (&query_branch, &update_reference)
                ))?;
            slog::debug!(
                log,
                "Updated old reference to replaced: {update_reference:?}"
            );
            // Silence all alerts for the old head reference
            let count = QueryAlert::silence_all(context, old_head_id).await?;
            slog::debug!(log, "Silenced {count} alerts for old reference");
        }

        // Get the updated branch
        // Make sure to do this after updating the old branch head reference to replaced
        let query_branch = QueryBranch::get(conn_lock!(context), query_branch.id)?;
        slog::debug!(log, "Got updated branch: {query_branch:?}");

        // Clone data from the start point for the head reference
        query_reference
            .clone_start_point(log, context, &query_branch, branch_start_point)
            .await?;
        slog::debug!(
            log,
            "Cloned start point for reference: {query_reference:?} {branch_start_point:?}"
        );

        Ok((query_branch, query_reference))
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = reference_table)]
pub struct UpdateReference {
    pub replaced: DateTime,
}

impl UpdateReference {
    pub fn replace() -> Self {
        Self {
            replaced: DateTime::now(),
        }
    }
}
