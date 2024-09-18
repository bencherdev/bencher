use bencher_json::{
    project::reference::{JsonVersion, VersionNumber},
    BranchUuid, DateTime, GitHash, JsonReference, JsonStartPoint, ReferenceUuid,
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
    version::{QueryVersion, VersionId},
    BranchId, BranchReferenceVersion, QueryBranch,
};
use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{issue_error, resource_conflict_err, resource_not_found_err},
    model::project::{
        threshold::{
            model::{InsertModel, QueryModel},
            InsertThreshold, QueryThreshold,
        },
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

    pub fn get_json(
        conn: &mut DbConnection,
        reference_id: ReferenceId,
        version: Option<QueryVersion>,
    ) -> Result<JsonReference, HttpError> {
        let (reference_uuid, branch_uuid, start_point_id, created, replaced) =
            schema::reference::table
                .inner_join(
                    schema::branch::table.on(schema::branch::id.eq(schema::reference::branch_id)),
                )
                .filter(schema::reference::id.eq(reference_id))
                .select((
                    schema::reference::uuid,
                    schema::branch::uuid,
                    schema::reference::start_point_id.nullable(),
                    schema::reference::created,
                    schema::reference::replaced.nullable(),
                ))
                .first::<(
                    ReferenceUuid,
                    BranchUuid,
                    Option<ReferenceVersionId>,
                    DateTime,
                    Option<DateTime>,
                )>(conn)
                .map_err(resource_not_found_err!(Reference, reference_id))?;

        let start_point = if let Some(start_point_id) = start_point_id {
            let (branch, reference, number, hash) = schema::reference_version::table
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
                reference,
                version: JsonVersion { number, hash },
            })
        } else {
            None
        };

        Ok(JsonReference {
            uuid: reference_uuid,
            branch: branch_uuid,
            start_point,
            version: version.map(QueryVersion::into_json),
            created,
            replaced,
        })
    }

    pub async fn clone_start_point(
        &self,
        context: &ApiContext,
        branch_start_point: Option<&BranchReferenceVersion>,
    ) -> Result<(), HttpError> {
        let branch_start_point = match (self.start_point_id, branch_start_point) {
            (Some(start_point_id), Some(branch_start_point)) => {
                debug_assert_eq!(start_point_id, branch_start_point.reference_version.id);
                branch_start_point
            },
            (None, None) => return Ok(()),
            _ => {
                return Err(issue_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Branch start point mismatch",
                    "Failed to match branch start point for reference",
                    format!("{branch_start_point:?}\n{self:?}"),
                ));
            },
        };
        self.clone_versions(context, &branch_start_point).await
    }

    async fn clone_versions(
        &self,
        context: &ApiContext,
        branch_start_point: &BranchReferenceVersion,
    ) -> Result<(), HttpError> {
        let start_point_version = QueryVersion::get(
            conn_lock!(context),
            branch_start_point.reference_version.version_id,
        )?;
        // Get all prior versions (version number less than or equal to) for the start point branch
        let version_ids = schema::reference_version::table
            .inner_join(schema::version::table)
            .filter(
                schema::reference_version::reference_id
                    .eq(branch_start_point.reference_version.reference_id),
            )
            .filter(schema::version::number.le(start_point_version.number))
            .order(schema::version::number.desc())
            .select(schema::reference_version::version_id)
            .load::<VersionId>(conn_lock!(context))
            .map_err(resource_not_found_err!(
                ReferenceVersion,
                (branch_start_point, start_point_version)
            ))?;

        // Add new branch to all start point branch versions
        for version_id in version_ids {
            let insert_branch_version = InsertReferenceVersion {
                reference_id: self.id,
                version_id,
            };
            diesel::insert_into(schema::reference_version::table)
                .values(&insert_branch_version)
                .execute(conn_lock!(context))
                .map_err(resource_conflict_err!(
                    ReferenceVersion,
                    insert_branch_version
                ))?;
        }

        Ok(())
    }

    async fn clone_thresholds(
        &self,
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        branch_start_point: &BranchReferenceVersion,
        new_branch: &QueryBranch,
    ) -> Result<(), HttpError> {
        // Get all thresholds for the start point branch
        let query_thresholds = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(branch_start_point.branch.id))
            .load::<QueryThreshold>(conn_lock!(context))
            .map_err(resource_not_found_err!(Threshold, branch_start_point))?;

        // Add new branch to cloned thresholds with cloned current threshold model
        for query_threshold in query_thresholds {
            // Hold the database lock across the entire `clone_threshold` call
            if let Err(e) = self.clone_threshold(
                conn_lock!(context),
                project_id,
                &query_threshold,
                new_branch,
            ) {
                slog::warn!(log, "Failed to clone threshold: {e}");
            }
        }

        Ok(())
    }

    fn clone_threshold(
        &self,
        conn: &mut DbConnection,
        project_id: ProjectId,
        query_threshold: &QueryThreshold,
        new_branch: &QueryBranch,
    ) -> Result<(), HttpError> {
        // Clone the threshold for the new branch
        let insert_threshold = InsertThreshold::new(
            project_id,
            new_branch.id,
            query_threshold.testbed_id,
            query_threshold.measure_id,
        );

        // Create the new threshold
        diesel::insert_into(schema::threshold::table)
            .values(&insert_threshold)
            .execute(conn)
            .map_err(resource_conflict_err!(Threshold, insert_threshold))?;

        // Get the new threshold
        let threshold_id = QueryThreshold::get_id(conn, insert_threshold.uuid)?;

        // Get the current threshold model
        let model_id = query_threshold.model_id()?;
        let query_model = schema::model::table
            .filter(schema::model::id.eq(model_id))
            .first::<QueryModel>(conn)
            .map_err(resource_not_found_err!(Model, query_threshold))?;

        // Clone the current threshold model with the new threshold ID
        let insert_model = InsertModel::with_threshold_id(query_model.clone(), threshold_id);
        // Create the new model for the new threshold
        diesel::insert_into(schema::model::table)
            .values(&insert_model)
            .execute(conn)
            .map_err(resource_conflict_err!(Model, insert_model))?;

        // Get the new model
        let model_id = QueryModel::get_id(conn, insert_model.uuid)?;

        // Set the new model for the new threshold
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)))
            .set(schema::threshold::model_id.eq(model_id))
            .execute(conn)
            .map_err(resource_conflict_err!(
                Threshold,
                (&query_threshold, &query_model)
            ))?;

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
        context: &ApiContext,
        query_branch: QueryBranch,
        branch_start_point: Option<BranchReferenceVersion>,
    ) -> Result<(QueryBranch, QueryReference), HttpError> {
        // Create the head reference for the branch
        let insert_reference = InsertReference::new(
            query_branch.id,
            branch_start_point
                .as_ref()
                .map(BranchReferenceVersion::reference_version_id),
        );
        diesel::insert_into(schema::reference::table)
            .values(&insert_reference)
            .execute(conn_lock!(context))
            .map_err(resource_conflict_err!(Reference, insert_reference))?;

        // Get the new reference
        let query_reference = schema::reference::table
            .filter(schema::reference::uuid.eq(&insert_reference.uuid))
            .first::<QueryReference>(conn_lock!(context))
            .map_err(resource_not_found_err!(Reference, insert_reference))?;

        // Update the branch head reference
        diesel::update(schema::branch::table.filter(schema::branch::id.eq(query_branch.id)))
            .set(schema::branch::head_id.eq(query_reference.id))
            .execute(conn_lock!(context))
            .map_err(resource_conflict_err!(
                Branch,
                (&query_branch, &query_reference)
            ))?;

        // Clone data from the start point for the head reference
        query_reference
            .clone_start_point(context, branch_start_point.as_ref())
            .await?;

        Ok((query_branch, query_reference))
    }
}
