use std::collections::HashMap;

use bencher_json::{
    DateTime, MetricName, Model, ParameterFilter, ParameterSet, ThresholdUuid,
    project::{
        report::JsonReportThresholds,
        threshold::{JsonThreshold, JsonThresholdModel},
    },
};
use diesel::{
    BelongingToDsl as _, ExpressionMethods as _, JoinOnDsl as _, NullableExpressionMethods as _,
    OptionalExtension as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _,
};
use dropshot::HttpError;
use model::UpdateModel;
use slog::Logger;

use self::model::{InsertModel, ModelId, QueryModel};
use super::{
    ProjectId, QueryProject,
    branch::{BranchId, QueryBranch, head::HeadId, start_point::StartPoint, version::VersionId},
    measure::{MeasureId, QueryMeasure},
    testbed::{QueryTestbed, TestbedId},
};
use crate::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::{
        BencherResource, assert_parentage, assert_siblings, resource_conflict_error,
        resource_not_found_err,
    },
    macros::{
        fn_get::{fn_get, fn_get_id, fn_get_uuid},
        sql::last_insert_rowid,
    },
    model::spec::SpecId,
    schema::{self, threshold as threshold_table},
    write_transaction,
};

pub mod alert;
pub mod boundary;
pub mod model;

crate::macros::typed_id::typed_id!(ThresholdId);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = threshold_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryThreshold {
    pub id: ThresholdId,
    pub uuid: ThresholdUuid,
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub testbed_id: TestbedId,
    pub measure_id: MeasureId,
    /// The name this threshold gates, when it is not the conventional `value` name.
    pub metric: Option<MetricName>,
    /// The grid points this threshold gates, when it does not gate every one.
    pub parameters: Option<ParameterFilter>,
    pub model_id: Option<ModelId>,
    pub created: DateTime,
    pub modified: DateTime,
}

/// The three dimensions a threshold hangs off.
///
/// They travel together because the identity travels beside them, and the two
/// halves of a threshold's unique key read better as two values than as five
/// positional arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThresholdDimensions {
    pub branch_id: BranchId,
    pub testbed_id: TestbedId,
    pub measure_id: MeasureId,
}

/// What a threshold gates beyond its branch, testbed, and measure: one metric name
/// and a filter over grid points.
///
/// Both halves are stored in their canonical form, which is the absence of a value
/// for the default: the conventional `value` name is a `NULL` metric and a filter
/// that matches every grid point is `NULL` parameters. The wire accepts either
/// spelling and this is what turns one into the other, so two spellings of one
/// threshold are one row.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ThresholdIdentity {
    pub metric: Option<MetricName>,
    pub parameters: Option<ParameterFilter>,
}

impl ThresholdIdentity {
    /// The canonical identity behind a metric name and a parameters filter.
    #[must_use]
    pub fn new(metric: Option<MetricName>, parameters: Option<ParameterFilter>) -> Self {
        Self {
            metric: metric.filter(|metric| *metric != MetricName::value()),
            parameters: parameters.filter(|parameters| !parameters.is_match_all()),
        }
    }

    /// The name this identity gates.
    #[must_use]
    pub fn metric_name(&self) -> MetricName {
        self.metric.clone().unwrap_or_else(MetricName::value)
    }

    /// Whether this identity gates every grid point of the conventional `value`
    /// name, which is what every threshold did before a threshold could name either.
    #[must_use]
    pub fn is_bare(&self) -> bool {
        self.metric.is_none() && self.parameters.is_none()
    }

    /// Whether this identity gates a grid point.
    #[must_use]
    pub fn matches(&self, grid_point: &ParameterSet) -> bool {
        self.parameters
            .as_ref()
            .is_none_or(|parameters| parameters.matches(grid_point))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ThresholdSpec {
    /// Use the testbed's own current spec (standalone threshold views)
    Testbed,
    /// Use the report's spec (alert views) — may be None for non-job reports
    Report(Option<SpecId>),
}

impl QueryThreshold {
    fn_get!(threshold, ThresholdId);
    fn_get_id!(threshold, ThresholdId, ThresholdUuid);
    fn_get_uuid!(threshold, ThresholdId, ThresholdUuid);

    /// What this threshold gates beyond its dimensions.
    #[must_use]
    pub fn identity(&self) -> ThresholdIdentity {
        ThresholdIdentity {
            metric: self.metric.clone(),
            parameters: self.parameters.clone(),
        }
    }

    /// The threshold with exactly this identity on these dimensions, if there is one.
    ///
    /// The identity is already canonical, so the two nullable columns are matched
    /// against the values they actually hold and never against a spelling of them.
    pub fn find_by_dimensions(
        conn: &mut DbConnection,
        branch_id: BranchId,
        testbed_id: TestbedId,
        measure_id: MeasureId,
        identity: &ThresholdIdentity,
    ) -> diesel::QueryResult<Option<Self>> {
        let mut query = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(branch_id))
            .filter(schema::threshold::testbed_id.eq(testbed_id))
            .filter(schema::threshold::measure_id.eq(measure_id))
            .into_boxed();
        query = if let Some(metric) = identity.metric.clone() {
            query.filter(schema::threshold::metric.eq(metric))
        } else {
            query.filter(schema::threshold::metric.is_null())
        };
        query = if let Some(parameters) = identity.parameters.clone() {
            query.filter(schema::threshold::parameters.eq(parameters))
        } else {
            query.filter(schema::threshold::parameters.is_null())
        };
        query.first::<Self>(conn).optional()
    }

    pub fn get_with_uuid(
        conn: &mut DbConnection,
        query_project: &QueryProject,
        uuid: ThresholdUuid,
    ) -> Result<Self, HttpError> {
        Self::belonging_to(&query_project)
            .filter(threshold_table::uuid.eq(uuid))
            .first::<Self>(conn)
            .map_err(resource_not_found_err!(Threshold, (query_project, uuid)))
    }

    pub fn model(&self, conn: &mut DbConnection) -> Result<Option<QueryModel>, HttpError> {
        if let Some(model_id) = self.model_id {
            Ok(Some(QueryModel::get(conn, model_id)?))
        } else {
            Ok(None)
        }
    }

    /// Compare the current model (if any) with a new model and return the appropriate action.
    fn compute_model_action(
        conn: &mut DbConnection,
        model_id: Option<ModelId>,
        new_model: Option<Model>,
    ) -> Result<ThresholdModelAction, HttpError> {
        Ok(match (model_id, new_model) {
            // No current model and no new model — nothing to do.
            (None, None) => ThresholdModelAction::NoChange,
            // No current model but a new model — insert it.
            (None, Some(model)) => ThresholdModelAction::Update(model),
            // Current model but no new model — remove it.
            (Some(_), None) => ThresholdModelAction::Remove,
            // Both present — update only if they differ.
            (Some(model_id), Some(model)) => {
                let current_model = QueryModel::get(conn, model_id)?.into_model();
                if current_model == model {
                    ThresholdModelAction::NoChange
                } else {
                    ThresholdModelAction::Update(model)
                }
            },
        })
    }

    pub async fn update_model_if_changed(
        &self,
        context: &ApiContext,
        model: Option<Model>,
    ) -> Result<(), HttpError> {
        match Self::compute_model_action(auth_conn!(context), self.model_id, model)? {
            ThresholdModelAction::NoChange => Ok(()),
            ThresholdModelAction::Update(model) => self.update_from_model(context, model).await,
            ThresholdModelAction::Remove => {
                write_transaction!(context, |conn| self.remove_current_model(conn)).map_err(|e| {
                    crate::error::issue_error(
                        "Failed to remove threshold model",
                        "Failed to remove threshold model:",
                        e,
                    )
                })
            },
        }
    }

    async fn update_from_model(&self, context: &ApiContext, model: Model) -> Result<(), HttpError> {
        #[cfg(feature = "plus")]
        InsertModel::rate_limit(context, self).await?;
        write_transaction!(context, |conn| self.update_from_model_inner(conn, model)).map_err(|e| {
            crate::error::issue_error(
                "Failed to update threshold model",
                "Failed to update threshold model:",
                e,
            )
        })
    }

    fn update_from_model_inner(
        &self,
        conn: &mut DbConnection,
        model: Model,
    ) -> diesel::QueryResult<()> {
        // Insert the new model
        let insert_model = InsertModel::new(self.id, model);
        diesel::insert_into(schema::model::table)
            .values(&insert_model)
            .execute(conn)?;

        // Get the new model ID and update the threshold
        let update_threshold = UpdateThreshold::new_model(conn)?;
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(self.id)))
            .set(&update_threshold)
            .execute(conn)?;

        self.update_replaced_model(conn, update_threshold.modified)
    }

    fn remove_current_model(&self, conn: &mut DbConnection) -> diesel::QueryResult<()> {
        // Skip if there is no current model
        if self.model_id.is_none() {
            return Ok(());
        }

        // Update the current threshold to remove the current model
        let update_threshold = UpdateThreshold::remove_model();
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(self.id)))
            .set(&update_threshold)
            .execute(conn)?;

        self.update_replaced_model(conn, update_threshold.modified)
    }

    fn update_replaced_model(
        &self,
        conn: &mut DbConnection,
        date_time: DateTime,
    ) -> diesel::QueryResult<()> {
        // Update the old model to be replaced, if there is one
        if let Some(model_id) = self.model_id {
            let update_model = UpdateModel::replaced_at(date_time);
            diesel::update(schema::model::table.filter(schema::model::id.eq(model_id)))
                .set(&update_model)
                .execute(conn)?;
        }
        Ok(())
    }

    pub fn get_alert_json(
        conn: &mut DbConnection,
        threshold_id: ThresholdId,
        model_id: ModelId,
        head_id: HeadId,
        version_id: VersionId,
        spec_id: Option<SpecId>,
    ) -> Result<JsonThreshold, HttpError> {
        let query_threshold = Self::get(conn, threshold_id)?;
        let query_model = QueryModel::get(conn, model_id)?;
        query_threshold.into_json_for_model(
            conn,
            Some(query_model),
            Some((head_id, version_id)),
            ThresholdSpec::Report(spec_id),
        )
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonThreshold, HttpError> {
        let query_model = self.model(conn)?;
        self.into_json_for_model(conn, query_model, None, ThresholdSpec::Testbed)
    }

    pub fn into_json_for_model(
        self,
        conn: &mut DbConnection,
        query_model: Option<QueryModel>,
        head_version: Option<(HeadId, VersionId)>,
        threshold_spec: ThresholdSpec,
    ) -> Result<JsonThreshold, HttpError> {
        let model = if let Some(query_model) = &query_model {
            assert_parentage(
                BencherResource::Threshold,
                self.id,
                BencherResource::Model,
                query_model.threshold_id,
            );
            Some(query_model.into_json(&self))
        } else {
            None
        };
        let Self {
            uuid,
            project_id,
            branch_id,
            testbed_id,
            measure_id,
            metric,
            parameters,
            created,
            modified,
            ..
        } = self;
        let query_project = QueryProject::get(conn, project_id)?;
        let branch = if let Some((head_id, version_id)) = head_version {
            QueryBranch::get_json_for_report(conn, &query_project, head_id, version_id)?
        } else {
            let query_branch = QueryBranch::get(conn, branch_id)?;
            query_branch.into_json_for_project(conn, &query_project)?
        };
        let testbed = match threshold_spec {
            ThresholdSpec::Report(spec_id) => {
                QueryTestbed::get_json_for_report(conn, &query_project, testbed_id, spec_id)?
            },
            ThresholdSpec::Testbed => {
                QueryTestbed::get(conn, testbed_id)?.into_json_for_project(conn, &query_project)?
            },
        };
        let measure = QueryMeasure::get(conn, measure_id)?.into_json_for_project(&query_project);
        Ok(JsonThreshold {
            uuid,
            project: query_project.uuid,
            branch,
            testbed,
            measure,
            metric,
            parameters,
            model,
            created,
            modified,
        })
    }

    pub fn into_threshold_model_json_for_project(
        self,
        project: &QueryProject,
        query_model: QueryModel,
    ) -> JsonThresholdModel {
        let model = query_model.into_json(&self);
        let Self {
            uuid,
            project_id,
            created,
            ..
        } = self;
        assert_parentage(
            BencherResource::Project,
            project.id,
            BencherResource::Threshold,
            project_id,
        );
        JsonThresholdModel {
            uuid,
            project: project.uuid,
            model,
            created,
        }
    }
}

#[derive(Debug, Clone, diesel::Insertable)]
#[diesel(table_name = threshold_table)]
pub struct InsertThreshold {
    pub uuid: ThresholdUuid,
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub testbed_id: TestbedId,
    pub measure_id: MeasureId,
    pub metric: Option<MetricName>,
    pub parameters: Option<ParameterFilter>,
    pub model_id: Option<ModelId>,
    pub created: DateTime,
    pub modified: DateTime,
}

/// The result of comparing a threshold's current model with a new model.
enum ThresholdModelAction {
    /// The new model differs from the current one — update it.
    Update(Model),
    /// There is a current model but no new model — remove it.
    Remove,
    /// The model is unchanged (or both are `None`) — nothing to do.
    NoChange,
}

enum StartPointAction {
    Create(TestbedId, MeasureId, ThresholdIdentity, Model),
    Update(QueryThreshold, Model),
    Remove(QueryThreshold),
    NoChange,
}

enum ThresholdAction {
    Create(MeasureId, Model),
    Update(QueryThreshold, Model),
    NoChange,
}

impl InsertThreshold {
    #[cfg(feature = "plus")]
    crate::macros::rate_limit::fn_rate_limit!(threshold, Threshold);

    pub fn new(
        project_id: ProjectId,
        branch_id: BranchId,
        testbed_id: TestbedId,
        measure_id: MeasureId,
        identity: ThresholdIdentity,
    ) -> Self {
        let timestamp = DateTime::now();
        let ThresholdIdentity { metric, parameters } = identity;
        Self {
            uuid: ThresholdUuid::new(),
            project_id,
            branch_id,
            testbed_id,
            measure_id,
            metric,
            parameters,
            model_id: None,
            created: timestamp,
            modified: timestamp,
        }
    }

    pub async fn from_model(
        context: &ApiContext,
        project_id: ProjectId,
        dimensions: ThresholdDimensions,
        identity: ThresholdIdentity,
        model: Model,
    ) -> Result<ThresholdId, HttpError> {
        let ThresholdDimensions {
            branch_id,
            testbed_id,
            measure_id,
        } = dimensions;
        // Check for an existing threshold with the same unique key before writing.
        // The key is the three dimensions and the identity together: a threshold that
        // gates `p99`, or that gates one corner of the grid, sits beside the bare one
        // rather than colliding with it.
        if let Some(existing) = QueryThreshold::find_by_dimensions(
            auth_conn!(context),
            branch_id,
            testbed_id,
            measure_id,
            &identity,
        )
        .map_err(|e| {
            crate::error::issue_error(
                "Failed to query threshold dimensions",
                "Failed to query threshold dimensions:",
                e,
            )
        })? {
            let metric = identity.metric_name();
            let parameters = identity
                .parameters
                .as_ref()
                .map_or_else(|| "every grid point".to_owned(), ParameterFilter::canonical);
            return Err(resource_conflict_error(
                BencherResource::Threshold,
                (branch_id, testbed_id, measure_id),
                format!(
                    "A threshold ({uuid}) already exists for this branch, testbed, and measure, gating {metric} of {parameters}",
                    uuid = existing.uuid
                ),
            ));
        }

        #[cfg(feature = "plus")]
        Self::rate_limit(context, project_id).await?;
        write_transaction!(context, |conn| {
            Self::from_model_inner(conn, project_id, dimensions, identity, model)
        })
        .map_err(|e| {
            crate::error::issue_error(
                "Failed to create threshold from model",
                "Failed to create threshold from model:",
                e,
            )
        })
    }

    fn from_model_inner(
        conn: &mut DbConnection,
        project_id: ProjectId,
        dimensions: ThresholdDimensions,
        identity: ThresholdIdentity,
        model: Model,
    ) -> diesel::QueryResult<ThresholdId> {
        let ThresholdDimensions {
            branch_id,
            testbed_id,
            measure_id,
        } = dimensions;
        // Create the new threshold
        let insert_threshold =
            InsertThreshold::new(project_id, branch_id, testbed_id, measure_id, identity);
        diesel::insert_into(schema::threshold::table)
            .values(&insert_threshold)
            .execute(conn)?;

        // Get the new threshold ID
        let threshold_id = diesel::select(last_insert_rowid()).get_result::<ThresholdId>(conn)?;

        // Create the new model
        let insert_model = InsertModel::new(threshold_id, model);
        diesel::insert_into(schema::model::table)
            .values(&insert_model)
            .execute(conn)?;

        // Get the new model ID and set it on the threshold
        let model_id = diesel::select(last_insert_rowid()).get_result::<ModelId>(conn)?;
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)))
            .set(schema::threshold::model_id.eq(model_id))
            .execute(conn)?;

        Ok(threshold_id)
    }

    pub async fn from_start_point(
        log: &Logger,
        context: &ApiContext,
        query_branch: &QueryBranch,
        branch_start_point: &StartPoint,
    ) -> Result<(), HttpError> {
        let Some(true) = branch_start_point.clone_thresholds else {
            slog::debug!(
                log,
                "Skipping cloning thresholds for start point: {branch_start_point:?}"
            );
            return Ok(());
        };

        assert_siblings(
            BencherResource::Project,
            BencherResource::Branch,
            query_branch.project_id,
            BencherResource::Branch,
            branch_start_point.branch.project_id,
        );

        // Phase 1: Read current and start point thresholds, pre-compute actions.
        let (actions, orphans) =
            Self::compute_start_point_actions(log, context, query_branch, branch_start_point)
                .await?;

        // Phase 2: Batch all writes in a single write lock + transaction.
        let has_writes = actions
            .iter()
            .any(|a| !matches!(a, StartPointAction::NoChange))
            || !orphans.is_empty();
        if has_writes {
            let project_id = query_branch.project_id;
            let branch_id = query_branch.id;
            write_transaction!(context, |conn| {
                for action in actions {
                    match action {
                        StartPointAction::Create(testbed_id, measure_id, identity, model) => {
                            InsertThreshold::from_model_inner(
                                conn,
                                project_id,
                                ThresholdDimensions {
                                    branch_id,
                                    testbed_id,
                                    measure_id,
                                },
                                identity,
                                model,
                            )?;
                        },
                        StartPointAction::Update(threshold, model) => {
                            threshold.update_from_model_inner(conn, model)?;
                        },
                        StartPointAction::Remove(threshold) => {
                            threshold.remove_current_model(conn)?;
                        },
                        StartPointAction::NoChange => {},
                    }
                }
                for threshold in orphans {
                    threshold.remove_current_model(conn)?;
                    slog::debug!(log, "Removed model from current threshold {threshold:?}",);
                }
                diesel::QueryResult::Ok(())
            })
            .map_err(|e| {
                crate::error::issue_error(
                    "Failed to sync start point thresholds",
                    "Failed to sync start point thresholds in batch transaction:",
                    e,
                )
            })?;
        }

        Ok(())
    }

    /// Phase 1 of start point sync: read thresholds and pre-compute actions.
    /// Rate limit checks happen here, before the write transaction.
    async fn compute_start_point_actions(
        log: &Logger,
        context: &ApiContext,
        query_branch: &QueryBranch,
        branch_start_point: &StartPoint,
    ) -> Result<(Vec<StartPointAction>, Vec<QueryThreshold>), HttpError> {
        let mut current_thresholds = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(query_branch.id))
            .load::<QueryThreshold>(auth_conn!(context))
            .map_err(resource_not_found_err!(
                Threshold,
                &branch_start_point.branch
            ))?
            .into_iter()
            // A branch may hold several thresholds on one (testbed, measure) now, so
            // the key a clone matches on is the whole of what a threshold gates.
            .map(|threshold| {
                (
                    (
                        threshold.testbed_id,
                        threshold.measure_id,
                        threshold.identity(),
                    ),
                    threshold,
                )
            })
            .collect::<HashMap<_, _>>();
        slog::debug!(log, "Current thresholds: {current_thresholds:?}");

        // Fetch start point thresholds with their models in a single JOIN query
        let start_point_thresholds = schema::threshold::table
            .left_join(
                schema::model::table
                    .on(schema::model::id.nullable().eq(schema::threshold::model_id)),
            )
            .filter(schema::threshold::branch_id.eq(branch_start_point.branch.id))
            .select((
                QueryThreshold::as_select(),
                Option::<QueryModel>::as_select(),
            ))
            .load::<(QueryThreshold, Option<QueryModel>)>(auth_conn!(context))
            .map_err(resource_not_found_err!(
                Threshold,
                &branch_start_point.branch
            ))?
            .into_iter()
            .map(|(threshold, model)| {
                (
                    (
                        threshold.testbed_id,
                        threshold.measure_id,
                        threshold.identity(),
                    ),
                    (threshold, model.map(QueryModel::into_model)),
                )
            })
            .collect::<HashMap<_, _>>();
        slog::debug!(log, "Start point thresholds: {start_point_thresholds:?}");

        // Pre-compute actions using read connections
        let auth_conn = auth_conn!(context);
        let mut actions = Vec::new();
        for (
            (start_point_testbed_id, start_point_measure_id, start_point_identity),
            (_start_point_threshold, start_point_model),
        ) in &start_point_thresholds
        {
            if let Some(current_threshold) = current_thresholds.remove(&(
                *start_point_testbed_id,
                *start_point_measure_id,
                start_point_identity.clone(),
            )) {
                match QueryThreshold::compute_model_action(
                    auth_conn,
                    current_threshold.model_id,
                    *start_point_model,
                )? {
                    ThresholdModelAction::NoChange => {
                        actions.push(StartPointAction::NoChange);
                    },
                    ThresholdModelAction::Update(model) => {
                        #[cfg(feature = "plus")]
                        InsertModel::rate_limit(context, &current_threshold).await?;
                        actions.push(StartPointAction::Update(current_threshold, model));
                    },
                    ThresholdModelAction::Remove => {
                        actions.push(StartPointAction::Remove(current_threshold));
                    },
                }
            } else if let Some(model) = start_point_model {
                #[cfg(feature = "plus")]
                Self::rate_limit(context, query_branch.project_id).await?;
                actions.push(StartPointAction::Create(
                    *start_point_testbed_id,
                    *start_point_measure_id,
                    start_point_identity.clone(),
                    *model,
                ));
            } else {
                actions.push(StartPointAction::NoChange);
            }
        }

        // Remaining current thresholds are orphans to remove
        let orphans: Vec<QueryThreshold> = current_thresholds.into_values().collect();
        slog::debug!(log, "Orphan thresholds to remove: {orphans:?}");

        Ok((actions, orphans))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "Batch threshold processing with rate limiting"
    )]
    pub async fn from_report_json(
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        branch_id: BranchId,
        testbed_id: TestbedId,
        json_thresholds: Option<JsonReportThresholds>,
    ) -> Result<(), HttpError> {
        #[cfg(feature = "plus")]
        Self::rate_limit(context, project_id).await?;

        let Some(json_thresholds) = json_thresholds else {
            slog::debug!(log, "No thresholds in report");
            return Ok(());
        };
        let no_models = json_thresholds
            .models
            .as_ref()
            .is_none_or(HashMap::is_empty);
        let reset_thresholds = json_thresholds.reset.unwrap_or_default();
        if no_models && !reset_thresholds {
            slog::debug!(log, "No threshold models or reset in report");
            return Ok(());
        }

        // Get all thresholds for the report branch and testbed (read phase).
        //
        // The map a report carries names a measure and a model and nothing else, so
        // the threshold it addresses is the bare one: the `value` name of every grid
        // point. A threshold that gates a name or a corner of the grid is addressed
        // through the thresholds endpoint, so it is not what this updates and not
        // what `reset` takes a model away from.
        let mut current_thresholds = schema::threshold::table
            .filter(schema::threshold::project_id.eq(project_id))
            .filter(schema::threshold::branch_id.eq(branch_id))
            .filter(schema::threshold::testbed_id.eq(testbed_id))
            .filter(schema::threshold::metric.is_null())
            .filter(schema::threshold::parameters.is_null())
            .load::<QueryThreshold>(auth_conn!(context))
            .map_err(resource_not_found_err!(Threshold, (branch_id, testbed_id)))?
            .into_iter()
            .map(|threshold| (threshold.measure_id, threshold))
            .collect::<HashMap<_, _>>();
        slog::debug!(log, "Current thresholds: {current_thresholds:?}");

        // Phase 1: Pre-resolve all measure IDs (may trigger get_or_create writes)
        // and read current model state.
        let auth_conn = auth_conn!(context);
        let mut actions = Vec::new();
        if let Some(models) = json_thresholds.models {
            for (measure, model) in models {
                let measure_id = QueryMeasure::get_or_create(context, project_id, &measure).await?;
                slog::debug!(log, "Processing threshold for measure {measure_id}");
                if let Some(current_threshold) = current_thresholds.remove(&measure_id) {
                    match QueryThreshold::compute_model_action(
                        auth_conn,
                        current_threshold.model_id,
                        Some(model),
                    )? {
                        ThresholdModelAction::Update(model) => {
                            #[cfg(feature = "plus")]
                            InsertModel::rate_limit(context, &current_threshold).await?;
                            slog::debug!(log, "Updating threshold for measure {measure_id}");
                            actions.push(ThresholdAction::Update(current_threshold, model));
                        },
                        ThresholdModelAction::NoChange => {
                            slog::debug!(log, "Model unchanged for measure {measure_id}");
                            actions.push(ThresholdAction::NoChange);
                        },
                        // Cannot happen: we always pass Some(model) as new_model.
                        ThresholdModelAction::Remove => {
                            return Err(crate::error::issue_error(
                                "Unexpected threshold model removal",
                                "compute_model_action returned Remove with Some(model) input for measure:",
                                measure_id,
                            ));
                        },
                    }
                } else {
                    slog::debug!(log, "Creating threshold for measure {measure_id}");
                    actions.push(ThresholdAction::Create(measure_id, model));
                }
            }
        }

        // Collect orphan thresholds to reset
        let orphans: Vec<QueryThreshold> = if reset_thresholds {
            current_thresholds.into_values().collect()
        } else {
            Vec::new()
        };

        // Phase 2: Batch all threshold writes in a single write lock acquisition
        // wrapped in a transaction for atomicity.
        // Skip if there's nothing to write.
        let has_writes = actions
            .iter()
            .any(|a| !matches!(a, ThresholdAction::NoChange))
            || !orphans.is_empty();
        if has_writes {
            write_transaction!(context, |conn| {
                for action in actions {
                    match action {
                        ThresholdAction::Create(measure_id, model) => {
                            InsertThreshold::from_model_inner(
                                conn,
                                project_id,
                                ThresholdDimensions {
                                    branch_id,
                                    testbed_id,
                                    measure_id,
                                },
                                ThresholdIdentity::default(),
                                model,
                            )?;
                        },
                        ThresholdAction::Update(threshold, model) => {
                            threshold.update_from_model_inner(conn, model)?;
                        },
                        ThresholdAction::NoChange => {},
                    }
                }
                for threshold in orphans {
                    threshold.remove_current_model(conn)?;
                    slog::debug!(log, "Removed model from threshold {threshold:?}");
                }
                diesel::QueryResult::Ok(())
            })
            .map_err(|e| {
                crate::error::issue_error(
                    "Failed to write report thresholds",
                    "Failed to write report thresholds in batch transaction:",
                    e,
                )
            })?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = threshold_table)]
pub struct UpdateThreshold {
    pub model_id: Option<Option<ModelId>>,
    pub modified: DateTime,
}

impl UpdateThreshold {
    /// Create an `UpdateThreshold` that sets the `model_id` to the most recently inserted model.
    ///
    /// # Precondition
    /// Must be called immediately after an `INSERT INTO model` on the same connection,
    /// within the same transaction. Uses `last_insert_rowid()` to retrieve the model ID.
    pub fn new_model(conn: &mut DbConnection) -> diesel::QueryResult<Self> {
        Ok(Self {
            model_id: Some(Some(
                diesel::select(last_insert_rowid()).get_result::<ModelId>(conn)?,
            )),
            modified: DateTime::now(),
        })
    }

    pub fn remove_model() -> Self {
        Self {
            model_id: Some(None),
            modified: DateTime::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use diesel::{
        ExpressionMethods as _, JoinOnDsl as _, NullableExpressionMethods as _, QueryDsl as _,
        RunQueryDsl as _, SelectableHelper as _,
    };

    use crate::{
        schema,
        test_util::{
            create_base_entities, create_branch_with_head, create_gating_threshold, create_measure,
            create_model, create_testbed, create_threshold, get_threshold_model_id,
            get_thresholds_for_branch, setup_test_db,
        },
    };

    use super::{
        InsertThreshold, MetricName, ParameterFilter, QueryThreshold, ThresholdDimensions,
        ThresholdId, ThresholdIdentity, UpdateThreshold, model::ModelId,
    };
    use crate::model::project::{ProjectId, measure::MeasureId, testbed::TestbedId};

    /// Test that thresholds can be queried by `branch_id`.
    /// This is the foundation of threshold cloning.
    #[test]
    fn query_thresholds_by_branch() {
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

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let threshold_id = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );

        let thresholds = get_thresholds_for_branch(&mut conn, branch.branch_id);
        assert_eq!(thresholds.len(), 1);
        assert_eq!(thresholds.first(), Some(&threshold_id));
    }

    /// Test threshold model relationship.
    /// Thresholds can optionally have a `model_id` pointing to a model.
    #[test]
    fn threshold_with_model() {
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

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let threshold_id = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );

        // Initially no model
        let model_id = get_threshold_model_id(&mut conn, threshold_id);
        assert!(model_id.is_none());

        // Add a model
        let model_id = create_model(
            &mut conn,
            threshold_id,
            "00000000-0000-0000-0000-000000000050",
            0, // test type
        );

        // Now threshold has a model
        let fetched_model_id = get_threshold_model_id(&mut conn, threshold_id);
        assert_eq!(fetched_model_id, Some(model_id));
    }

    /// Test threshold without model.
    /// Thresholds can exist without models.
    #[test]
    fn threshold_without_model() {
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

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let threshold_id = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );

        let model_id = get_threshold_model_id(&mut conn, threshold_id);
        assert!(model_id.is_none());
    }

    /// Test collecting thresholds into a `HashMap` by (`testbed_id`, `measure_id`).
    /// This is how `from_start_point` organizes thresholds for matching.
    #[test]
    fn threshold_hashmap_by_testbed_measure() {
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

        let testbed1 = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let testbed2 = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000021",
            "ci-runner",
            "ci-runner",
        );

        let measure1 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let measure2 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000031",
            "throughput",
            "throughput",
        );

        // Create thresholds for different testbed/measure combinations
        let t1 = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed1,
            measure1,
            "00000000-0000-0000-0000-000000000040",
        );
        let t2 = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed1,
            measure2,
            "00000000-0000-0000-0000-000000000041",
        );
        let t3 = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed2,
            measure1,
            "00000000-0000-0000-0000-000000000042",
        );

        // Query and collect into HashMap like from_start_point does
        let thresholds: HashMap<(TestbedId, MeasureId), ThresholdId> = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(branch.branch_id))
            .select((
                schema::threshold::testbed_id,
                schema::threshold::measure_id,
                schema::threshold::id,
            ))
            .load::<(TestbedId, MeasureId, ThresholdId)>(&mut conn)
            .expect("Failed to query")
            .into_iter()
            .map(|(testbed_id, measure_id, id)| ((testbed_id, measure_id), id))
            .collect();

        assert_eq!(thresholds.len(), 3);
        assert_eq!(thresholds.get(&(testbed1, measure1)), Some(&t1));
        assert_eq!(thresholds.get(&(testbed1, measure2)), Some(&t2));
        assert_eq!(thresholds.get(&(testbed2, measure1)), Some(&t3));
    }

    /// Test matching thresholds between branches by (`testbed_id`, `measure_id`).
    /// This simulates the core matching logic in `from_start_point`.
    #[test]
    #[expect(clippy::too_many_lines, reason = "Test setup requires many entities")]
    fn threshold_matching_between_branches() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        // Source branch
        let source = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        // Destination branch
        let dest = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000012",
            "feature",
            "feature",
            "00000000-0000-0000-0000-000000000013",
        );

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure1 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let measure2 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000031",
            "throughput",
            "throughput",
        );

        // Source has thresholds for measure1 and measure2
        create_threshold(
            &mut conn,
            base.project_id,
            source.branch_id,
            testbed,
            measure1,
            "00000000-0000-0000-0000-000000000040",
        );
        create_threshold(
            &mut conn,
            base.project_id,
            source.branch_id,
            testbed,
            measure2,
            "00000000-0000-0000-0000-000000000041",
        );

        // Dest only has threshold for measure1
        create_threshold(
            &mut conn,
            base.project_id,
            dest.branch_id,
            testbed,
            measure1,
            "00000000-0000-0000-0000-000000000050",
        );

        // Collect source thresholds
        let source_thresholds: HashMap<(TestbedId, MeasureId), ThresholdId> =
            schema::threshold::table
                .filter(schema::threshold::branch_id.eq(source.branch_id))
                .select((
                    schema::threshold::testbed_id,
                    schema::threshold::measure_id,
                    schema::threshold::id,
                ))
                .load::<(TestbedId, MeasureId, ThresholdId)>(&mut conn)
                .expect("Failed to query")
                .into_iter()
                .map(|(testbed_id, measure_id, id)| ((testbed_id, measure_id), id))
                .collect();

        // Collect dest thresholds
        let mut dest_thresholds: HashMap<(TestbedId, MeasureId), ThresholdId> =
            schema::threshold::table
                .filter(schema::threshold::branch_id.eq(dest.branch_id))
                .select((
                    schema::threshold::testbed_id,
                    schema::threshold::measure_id,
                    schema::threshold::id,
                ))
                .load::<(TestbedId, MeasureId, ThresholdId)>(&mut conn)
                .expect("Failed to query")
                .into_iter()
                .map(|(testbed_id, measure_id, id)| ((testbed_id, measure_id), id))
                .collect();

        assert_eq!(source_thresholds.len(), 2);
        assert_eq!(dest_thresholds.len(), 1);

        // Simulate from_start_point matching logic
        let mut matched = Vec::new();
        let mut new_thresholds_needed = Vec::new();

        for (testbed_id, measure_id) in source_thresholds.keys() {
            if let Some(dest_threshold_id) = dest_thresholds.remove(&(*testbed_id, *measure_id)) {
                matched.push(dest_threshold_id);
            } else {
                new_thresholds_needed.push((*testbed_id, *measure_id));
            }
        }

        // One matched (measure1), one new needed (measure2)
        assert_eq!(matched.len(), 1);
        assert_eq!(new_thresholds_needed.len(), 1);
        assert_eq!(new_thresholds_needed.first(), Some(&(testbed, measure2)));

        // No orphans in dest (dest_thresholds is now empty after remove)
        assert!(dest_thresholds.is_empty());
    }

    /// Test orphan threshold detection.
    /// Dest thresholds not in source should be identified as orphans.
    #[test]
    fn orphan_threshold_detection() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let source = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        let dest = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000012",
            "feature",
            "feature",
            "00000000-0000-0000-0000-000000000013",
        );

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure1 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let measure2 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000031",
            "throughput",
            "throughput",
        );

        // Source has only measure1
        create_threshold(
            &mut conn,
            base.project_id,
            source.branch_id,
            testbed,
            measure1,
            "00000000-0000-0000-0000-000000000040",
        );

        // Dest has both measure1 and measure2
        create_threshold(
            &mut conn,
            base.project_id,
            dest.branch_id,
            testbed,
            measure1,
            "00000000-0000-0000-0000-000000000050",
        );
        let orphan_threshold_id = create_threshold(
            &mut conn,
            base.project_id,
            dest.branch_id,
            testbed,
            measure2,
            "00000000-0000-0000-0000-000000000051",
        );

        // Collect source thresholds
        let source_thresholds: HashMap<(TestbedId, MeasureId), ThresholdId> =
            schema::threshold::table
                .filter(schema::threshold::branch_id.eq(source.branch_id))
                .select((
                    schema::threshold::testbed_id,
                    schema::threshold::measure_id,
                    schema::threshold::id,
                ))
                .load::<(TestbedId, MeasureId, ThresholdId)>(&mut conn)
                .expect("Failed to query")
                .into_iter()
                .map(|(testbed_id, measure_id, id)| ((testbed_id, measure_id), id))
                .collect();

        // Collect dest thresholds
        let mut dest_thresholds: HashMap<(TestbedId, MeasureId), ThresholdId> =
            schema::threshold::table
                .filter(schema::threshold::branch_id.eq(dest.branch_id))
                .select((
                    schema::threshold::testbed_id,
                    schema::threshold::measure_id,
                    schema::threshold::id,
                ))
                .load::<(TestbedId, MeasureId, ThresholdId)>(&mut conn)
                .expect("Failed to query")
                .into_iter()
                .map(|(testbed_id, measure_id, id)| ((testbed_id, measure_id), id))
                .collect();

        // Process source thresholds (removes matching from dest)
        for (testbed_id, measure_id) in source_thresholds.keys() {
            dest_thresholds.remove(&(*testbed_id, *measure_id));
        }

        // Remaining dest_thresholds are orphans
        assert_eq!(dest_thresholds.len(), 1);
        assert!(
            dest_thresholds
                .values()
                .any(|&id| id == orphan_threshold_id)
        );
    }

    /// Test that `InsertThreshold` creates valid records.
    #[test]
    fn insert_threshold() {
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

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        // Use create_threshold helper which does the insertion
        let threshold_id = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );

        let threshold: QueryThreshold = schema::threshold::table
            .filter(schema::threshold::id.eq(threshold_id))
            .first(&mut conn)
            .expect("Failed to query threshold");

        assert_eq!(threshold.project_id, base.project_id);
        assert_eq!(threshold.branch_id, branch.branch_id);
        assert_eq!(threshold.testbed_id, testbed);
        assert_eq!(threshold.measure_id, measure);
        assert!(threshold.model_id.is_none());
    }

    /// Test removing model from threshold.
    /// The `UpdateThreshold::remove_model()` sets `model_id` to `None`.
    #[test]
    fn remove_model_from_threshold() {
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

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let threshold_id = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );

        // Add a model
        create_model(
            &mut conn,
            threshold_id,
            "00000000-0000-0000-0000-000000000050",
            0,
        );

        // Verify model exists
        assert!(get_threshold_model_id(&mut conn, threshold_id).is_some());

        // Remove model using UpdateThreshold
        let update_threshold = UpdateThreshold::remove_model();
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)))
            .set(&update_threshold)
            .execute(&mut conn)
            .expect("Failed to update threshold");

        // Verify model is removed
        assert!(get_threshold_model_id(&mut conn, threshold_id).is_none());
    }

    /// Test threshold model relationship via JOIN query.
    /// This tests the LEFT JOIN pattern used in the optimized `from_start_point`.
    #[test]
    fn threshold_model_join_query() {
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

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure1 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let measure2 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000031",
            "throughput",
            "throughput",
        );

        // Threshold 1 with model
        let t1 = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure1,
            "00000000-0000-0000-0000-000000000040",
        );
        create_model(&mut conn, t1, "00000000-0000-0000-0000-000000000050", 0);

        // Threshold 2 without model
        let t2 = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure2,
            "00000000-0000-0000-0000-000000000041",
        );

        // Use LEFT JOIN to get thresholds with optional models

        let results: Vec<(ThresholdId, Option<ModelId>)> = schema::threshold::table
            .left_join(
                schema::model::table
                    .on(schema::model::id.nullable().eq(schema::threshold::model_id)),
            )
            .filter(schema::threshold::branch_id.eq(branch.branch_id))
            .select((schema::threshold::id, schema::model::id.nullable()))
            .load(&mut conn)
            .expect("Failed to query");

        assert_eq!(results.len(), 2);

        // Find results by threshold id
        let t1_result = results.iter().find(|(id, _)| *id == t1);
        let t2_result = results.iter().find(|(id, _)| *id == t2);

        assert!(t1_result.unwrap().1.is_some()); // t1 has model
        assert!(t2_result.unwrap().1.is_none()); // t2 has no model
    }

    /// Test multiple thresholds with models using JOIN.
    /// Ensures the JOIN pattern works correctly with multiple thresholds.
    #[test]
    fn multiple_thresholds_with_models_join() {
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

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        // Create 5 thresholds, 3 with models, 2 without
        let mut thresholds_with_models = Vec::new();
        let mut thresholds_without_models = Vec::new();

        for i in 0..5 {
            let uuid_suffix = 30 + i;
            let measure = create_measure(
                &mut conn,
                base.project_id,
                &format!("00000000-0000-0000-0000-0000000000{uuid_suffix:02}"),
                &format!("measure{i}"),
                &format!("measure{i}"),
            );

            let threshold_suffix = 40 + i;
            let threshold_id = create_threshold(
                &mut conn,
                base.project_id,
                branch.branch_id,
                testbed,
                measure,
                &format!("00000000-0000-0000-0000-0000000000{threshold_suffix:02}"),
            );

            if i < 3 {
                let model_suffix = 50 + i;
                create_model(
                    &mut conn,
                    threshold_id,
                    &format!("00000000-0000-0000-0000-0000000000{model_suffix:02}"),
                    0,
                );
                thresholds_with_models.push(threshold_id);
            } else {
                thresholds_without_models.push(threshold_id);
            }
        }

        // Use LEFT JOIN to fetch all at once

        let results: Vec<(ThresholdId, Option<ModelId>)> = schema::threshold::table
            .left_join(
                schema::model::table
                    .on(schema::model::id.nullable().eq(schema::threshold::model_id)),
            )
            .filter(schema::threshold::branch_id.eq(branch.branch_id))
            .select((schema::threshold::id, schema::model::id.nullable()))
            .load(&mut conn)
            .expect("Failed to query");

        assert_eq!(results.len(), 5);

        let with_model_count = results.iter().filter(|(_, m)| m.is_some()).count();
        let without_model_count = results.iter().filter(|(_, m)| m.is_none()).count();

        assert_eq!(with_model_count, 3);
        assert_eq!(without_model_count, 2);

        // Verify specific thresholds
        for t in &thresholds_with_models {
            let result = results.iter().find(|(id, _)| id == t).unwrap();
            assert!(result.1.is_some());
        }

        for t in &thresholds_without_models {
            let result = results.iter().find(|(id, _)| id == t).unwrap();
            assert!(result.1.is_none());
        }
    }

    /// Test the complete `from_start_point` matching algorithm simulation.
    /// This simulates all cases: update existing, create new, remove orphan.
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "Comprehensive test requires many entities"
    )]
    fn from_start_point_matching_simulation() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        // Source branch (start point)
        let source = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        // Current branch
        let current = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000012",
            "feature",
            "feature",
            "00000000-0000-0000-0000-000000000013",
        );

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure_shared = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "shared",
            "shared",
        );

        let measure_source_only = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000031",
            "source_only",
            "source-only",
        );

        let measure_current_only = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000032",
            "current_only",
            "current-only",
        );

        // Source thresholds: shared (with model), source_only (with model)
        let source_shared = create_threshold(
            &mut conn,
            base.project_id,
            source.branch_id,
            testbed,
            measure_shared,
            "00000000-0000-0000-0000-000000000040",
        );
        create_model(
            &mut conn,
            source_shared,
            "00000000-0000-0000-0000-000000000060",
            0,
        );

        let source_only = create_threshold(
            &mut conn,
            base.project_id,
            source.branch_id,
            testbed,
            measure_source_only,
            "00000000-0000-0000-0000-000000000041",
        );
        create_model(
            &mut conn,
            source_only,
            "00000000-0000-0000-0000-000000000061",
            0,
        );

        // Current thresholds: shared (with model), current_only (with model)
        let current_shared = create_threshold(
            &mut conn,
            base.project_id,
            current.branch_id,
            testbed,
            measure_shared,
            "00000000-0000-0000-0000-000000000050",
        );
        create_model(
            &mut conn,
            current_shared,
            "00000000-0000-0000-0000-000000000070",
            0,
        );

        let current_only = create_threshold(
            &mut conn,
            base.project_id,
            current.branch_id,
            testbed,
            measure_current_only,
            "00000000-0000-0000-0000-000000000051",
        );
        create_model(
            &mut conn,
            current_only,
            "00000000-0000-0000-0000-000000000071",
            0,
        );

        // Simulate from_start_point logic
        let source_thresholds: HashMap<(TestbedId, MeasureId), (ThresholdId, Option<ModelId>)> =
            schema::threshold::table
                .filter(schema::threshold::branch_id.eq(source.branch_id))
                .select((
                    schema::threshold::testbed_id,
                    schema::threshold::measure_id,
                    schema::threshold::id,
                    schema::threshold::model_id,
                ))
                .load::<(TestbedId, MeasureId, ThresholdId, Option<ModelId>)>(&mut conn)
                .expect("Failed to query")
                .into_iter()
                .map(|(t, m, id, model)| ((t, m), (id, model)))
                .collect();

        let mut current_thresholds: HashMap<(TestbedId, MeasureId), ThresholdId> =
            schema::threshold::table
                .filter(schema::threshold::branch_id.eq(current.branch_id))
                .select((
                    schema::threshold::testbed_id,
                    schema::threshold::measure_id,
                    schema::threshold::id,
                ))
                .load::<(TestbedId, MeasureId, ThresholdId)>(&mut conn)
                .expect("Failed to query")
                .into_iter()
                .map(|(t, m, id)| ((t, m), id))
                .collect();

        assert_eq!(source_thresholds.len(), 2);
        assert_eq!(current_thresholds.len(), 2);

        let mut to_update = Vec::new();
        let mut to_create = Vec::new();

        for ((testbed_id, measure_id), (_source_id, source_model)) in &source_thresholds {
            if let Some(current_id) = current_thresholds.remove(&(*testbed_id, *measure_id)) {
                // Match found - would update
                to_update.push((current_id, *source_model));
            } else if source_model.is_some() {
                // No match but source has model - would create
                to_create.push((*testbed_id, *measure_id));
            }
        }

        // Remaining current_thresholds are orphans
        let orphans: Vec<_> = current_thresholds.values().copied().collect();

        // Verify expected behavior
        assert_eq!(to_update.len(), 1); // shared
        assert_eq!(to_create.len(), 1); // source_only
        assert_eq!(orphans.len(), 1); // current_only

        // Verify specific thresholds
        assert_eq!(to_update.first().map(|(id, _)| *id), Some(current_shared));
        assert_eq!(to_create.first(), Some(&(testbed, measure_source_only)));
        assert_eq!(orphans.first(), Some(&current_only));
    }

    /// Test that thresholds from different branches are isolated.
    /// Queries should only return thresholds for the specified branch.
    #[test]
    fn threshold_branch_isolation() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let branch1 = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "branch1",
            "branch1",
            "00000000-0000-0000-0000-000000000011",
        );

        let branch2 = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000012",
            "branch2",
            "branch2",
            "00000000-0000-0000-0000-000000000013",
        );

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        // Create threshold for branch1
        let t1 = create_threshold(
            &mut conn,
            base.project_id,
            branch1.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );

        // Create threshold for branch2
        let t2 = create_threshold(
            &mut conn,
            base.project_id,
            branch2.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000041",
        );

        // Query branch1 thresholds
        let branch1_thresholds = get_thresholds_for_branch(&mut conn, branch1.branch_id);
        assert_eq!(branch1_thresholds.len(), 1);
        assert_eq!(branch1_thresholds.first(), Some(&t1));

        // Query branch2 thresholds
        let branch2_thresholds = get_thresholds_for_branch(&mut conn, branch2.branch_id);
        assert_eq!(branch2_thresholds.len(), 1);
        assert_eq!(branch2_thresholds.first(), Some(&t2));
    }

    /// Test large threshold set with models.
    /// Ensures the system can handle many thresholds efficiently.
    #[test]
    fn large_threshold_set() {
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

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        // Create 20 thresholds with models
        for i in 0..20 {
            let uuid_suffix = 30 + i;
            let measure = create_measure(
                &mut conn,
                base.project_id,
                &format!("00000000-0000-0000-0000-0000000000{uuid_suffix:02}"),
                &format!("measure{i}"),
                &format!("measure{i}"),
            );

            let threshold_suffix = 40 + i;
            let threshold_id = create_threshold(
                &mut conn,
                base.project_id,
                branch.branch_id,
                testbed,
                measure,
                &format!("00000000-0000-0000-0000-0000000000{threshold_suffix:02}"),
            );

            let model_suffix = 60 + i;
            create_model(
                &mut conn,
                threshold_id,
                &format!("00000000-0000-0000-0000-0000000000{model_suffix:02}"),
                0,
            );
        }

        let thresholds = get_thresholds_for_branch(&mut conn, branch.branch_id);
        assert_eq!(thresholds.len(), 20);

        // Verify all have models using JOIN query

        let results: Vec<(ThresholdId, Option<ModelId>)> = schema::threshold::table
            .left_join(
                schema::model::table
                    .on(schema::model::id.nullable().eq(schema::threshold::model_id)),
            )
            .filter(schema::threshold::branch_id.eq(branch.branch_id))
            .select((schema::threshold::id, schema::model::id.nullable()))
            .load(&mut conn)
            .expect("Failed to query");

        assert_eq!(results.len(), 20);
        assert!(results.iter().all(|(_, m)| m.is_some()));
    }

    /// Test that `find_by_dimensions` returns `None` when no threshold exists.
    #[test]
    fn find_by_dimensions_returns_none_when_no_threshold() {
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

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let result = QueryThreshold::find_by_dimensions(
            &mut conn,
            branch.branch_id,
            testbed,
            measure,
            &ThresholdIdentity::default(),
        )
        .expect("Query should succeed");
        assert!(result.is_none());
    }

    /// Test that `find_by_dimensions` returns the existing threshold.
    #[test]
    fn find_by_dimensions_returns_existing_threshold() {
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

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let threshold_id = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );

        let result = QueryThreshold::find_by_dimensions(
            &mut conn,
            branch.branch_id,
            testbed,
            measure,
            &ThresholdIdentity::default(),
        )
        .expect("Query should succeed");
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, threshold_id);
    }

    /// Test that inserting a duplicate threshold violates the UNIQUE constraint.
    #[test]
    fn duplicate_threshold_insert_fails_with_unique_constraint() {
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

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        // First insert succeeds
        create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );

        // Second insert with same (branch, testbed, measure) should fail
        let result = diesel::insert_into(schema::threshold::table)
            .values((
                schema::threshold::uuid.eq("00000000-0000-0000-0000-000000000041"),
                schema::threshold::project_id.eq(base.project_id),
                schema::threshold::branch_id.eq(branch.branch_id),
                schema::threshold::testbed_id.eq(testbed),
                schema::threshold::measure_id.eq(measure),
                schema::threshold::created.eq(bencher_json::DateTime::TEST),
                schema::threshold::modified.eq(bencher_json::DateTime::TEST),
            ))
            .execute(&mut conn);

        result.unwrap_err();
    }

    fn metric_name(name: &str) -> MetricName {
        name.parse().expect("Invalid metric name")
    }

    fn filter(filter: &str) -> ParameterFilter {
        filter.parse().expect("Invalid parameters filter")
    }

    /// The dimensions every identity test hangs off.
    fn identity_fixture(
        conn: &mut crate::context::DbConnection,
    ) -> (ProjectId, ThresholdDimensions) {
        let base = create_base_entities(conn);
        let branch = create_branch_with_head(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );
        let testbed = create_testbed(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );
        let measure = create_measure(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );
        (
            base.project_id,
            ThresholdDimensions {
                branch_id: branch.branch_id,
                testbed_id: testbed,
                measure_id: measure,
            },
        )
    }

    /// The conventional `value` name spelled out is the same identity as no name at
    /// all, and the canonical form of both is no name at all.
    #[test]
    fn identity_collapses_the_value_name() {
        let bare = ThresholdIdentity::new(None, None);
        let explicit = ThresholdIdentity::new(Some(metric_name("value")), None);
        assert_eq!(bare, explicit);
        assert!(explicit.metric.is_none());
        assert!(explicit.is_bare());
        assert_eq!(explicit.metric_name(), MetricName::value());

        let named = ThresholdIdentity::new(Some(metric_name("p99")), None);
        assert_ne!(bare, named);
        assert!(!named.is_bare());
        assert_eq!(named.metric_name(), metric_name("p99"));
    }

    /// A filter that matches every grid point is the same identity as no filter, in
    /// either of the two spellings that say so.
    #[test]
    fn identity_collapses_a_match_all_filter() {
        let bare = ThresholdIdentity::new(None, None);
        assert_eq!(bare, ThresholdIdentity::new(None, Some(filter("[]"))));
        assert_eq!(bare, ThresholdIdentity::new(None, Some(filter("[{}]"))));
        // A filter holding the empty set matches everything the rest of it could
        // have narrowed, so it is match all too.
        assert_eq!(
            bare,
            ThresholdIdentity::new(None, Some(filter(r#"[{"size": 512}, {}]"#)))
        );

        let filtered = ThresholdIdentity::new(None, Some(filter(r#"[{"size": 512}]"#)));
        assert_ne!(bare, filtered);
        assert!(!filtered.is_bare());
    }

    /// One filter has one canonical spelling: the sets sort by their canonical bytes
    /// and duplicates collapse, so a number written two ways is one set.
    #[test]
    fn identity_canonicalizes_the_filter() {
        let one_way = filter(r#"[{"a": 1}, {"a": 1.0}]"#);
        assert_eq!(one_way.sets().len(), 1);
        assert_eq!(one_way.canonical(), r#"[{"a":1}]"#);

        let sorted = filter(r#"[{"b": 2}, {"a": 1}]"#);
        assert_eq!(sorted.canonical(), r#"[{"a":1},{"b":2}]"#);
        assert_eq!(sorted, filter(r#"[{"a": 1}, {"b": 2}]"#));
    }

    /// Two thresholds that gate the same name of the same grid points collide,
    /// whichever way each spells it.
    #[test]
    fn duplicate_identity_insert_fails_at_both_spellings() {
        let mut conn = setup_test_db();
        let (project_id, dimensions) = identity_fixture(&mut conn);
        let ThresholdDimensions {
            branch_id,
            testbed_id,
            measure_id,
        } = dimensions;

        // A bare threshold and one that spells out everything it defaults to.
        create_threshold(
            &mut conn,
            project_id,
            branch_id,
            testbed_id,
            measure_id,
            "00000000-0000-0000-0000-000000000040",
        );
        let spelled_out = ThresholdIdentity::new(Some(metric_name("value")), Some(filter("[{}]")));
        let insert = InsertThreshold::new(
            project_id,
            branch_id,
            testbed_id,
            measure_id,
            spelled_out.clone(),
        );
        diesel::insert_into(schema::threshold::table)
            .values(&insert)
            .execute(&mut conn)
            .unwrap_err();
        // The canonical form of the spelled out identity is the bare one, which is
        // what makes the two collide.
        assert!(spelled_out.is_bare());

        // A named and filtered threshold sits beside the bare one.
        let filtered =
            ThresholdIdentity::new(Some(metric_name("p99")), Some(filter(r#"[{"size": 512}]"#)));
        let insert = InsertThreshold::new(
            project_id,
            branch_id,
            testbed_id,
            measure_id,
            filtered.clone(),
        );
        diesel::insert_into(schema::threshold::table)
            .values(&insert)
            .execute(&mut conn)
            .expect("A filtered threshold sits beside the bare one");

        // But a second one that spells the same filter differently does not.
        let restated = ThresholdIdentity::new(
            Some(metric_name("p99")),
            Some(filter(r#"[{"size": 512.0}, {"size": 512}]"#)),
        );
        assert_eq!(filtered, restated);
        let insert = InsertThreshold::new(project_id, branch_id, testbed_id, measure_id, restated);
        diesel::insert_into(schema::threshold::table)
            .values(&insert)
            .execute(&mut conn)
            .unwrap_err();
    }

    /// A stored filter round trips through the column byte for byte.
    #[test]
    fn filter_round_trips_through_the_column() {
        let mut conn = setup_test_db();
        let (project_id, dimensions) = identity_fixture(&mut conn);
        let ThresholdDimensions {
            branch_id,
            testbed_id,
            measure_id,
        } = dimensions;

        let stored = filter(r#"[{"threads": 4, "size": 1024}, {"size": 512}]"#);
        create_gating_threshold(
            &mut conn,
            project_id,
            branch_id,
            testbed_id,
            measure_id,
            "00000000-0000-0000-0000-000000000040",
            Some(metric_name("p99")),
            Some(stored.clone()),
        );

        let read = schema::threshold::table
            .select(QueryThreshold::as_select())
            .first::<QueryThreshold>(&mut conn)
            .expect("Failed to read the threshold");
        assert_eq!(read.metric, Some(metric_name("p99")));
        assert_eq!(read.parameters, Some(stored));
        // Canonical bytes, not numeric value: `{"size":1` sorts before `{"size":5`.
        assert_eq!(
            read.identity().parameters.expect("No filter").canonical(),
            r#"[{"size":1024,"threads":4},{"size":512}]"#
        );
    }
}
