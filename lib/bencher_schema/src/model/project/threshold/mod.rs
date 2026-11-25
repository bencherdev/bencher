use std::collections::{HashMap, HashSet, hash_map::Entry};
#[cfg(feature = "plus")]
use std::num::NonZeroU32;

use bencher_json::{
    BmfVersion, DateTime, JsonModel, MetricName, Model, ModelSlot, ParameterFilter, ThresholdUuid,
    project::{
        report::{
            JsonReportModel, JsonReportThresholdEntry, JsonReportThresholdEntryModel,
            JsonReportThresholdModels, JsonReportThresholds, ReportWarningResource,
        },
        threshold::{JsonThreshold, JsonThresholdModel, MAX_ACTIVE_THRESHOLDS},
    },
};
use diesel::{
    BelongingToDsl as _, BoolExpressionMethods as _, ExpressionMethods as _, JoinOnDsl as _,
    NullableExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _,
    SelectableHelper as _,
};
#[cfg(feature = "plus")]
use dropshot::ClientErrorStatusCode;
use dropshot::HttpError;
use model::UpdateModel;
use slog::Logger;

use self::model::{InsertModel, ModelId, QueryModel};
use super::{
    ProjectId, QueryProject,
    branch::{BranchId, QueryBranch, head::HeadId, start_point::StartPoint, version::VersionId},
    measure::{MeasureId, QueryMeasure},
    report::warning::ReportWarnings,
    testbed::{QueryTestbed, TestbedId},
};
use crate::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::{
        BencherResource, assert_parentage, assert_siblings, bad_request_error, past_the_ceiling,
        resource_conflict_error, resource_not_found_err,
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
#[cfg(feature = "plus")]
pub mod changepoint;
#[cfg(feature = "plus")]
pub mod changepoint_scan;
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
    /// The variants this threshold checks, when it does not check every one.
    pub parameters: Option<ParameterFilter>,
    pub measure_id: MeasureId,
    /// The name this threshold checks, when it is not the conventional `value` name.
    pub metric: Option<MetricName>,
    pub model_id: Option<ModelId>,
    pub changepoint_model_id: Option<ModelId>,
    pub created: DateTime,
    pub modified: DateTime,
}

/// The dimensions a threshold hangs off, which together are its unique key.
///
/// The two nullable dimensions are canonical: the default spelling of each is its
/// absence, and [`Self::new`] is what turns one into the other.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThresholdDimensions {
    pub branch_id: BranchId,
    pub testbed_id: TestbedId,
    pub parameters: Option<ParameterFilter>,
    pub measure_id: MeasureId,
    pub metric: Option<MetricName>,
}

impl ThresholdDimensions {
    #[must_use]
    pub fn new(
        branch_id: BranchId,
        testbed_id: TestbedId,
        parameters: Option<ParameterFilter>,
        measure_id: MeasureId,
        metric: Option<MetricName>,
    ) -> Self {
        Self {
            branch_id,
            testbed_id,
            parameters: parameters.filter(|parameters| !parameters.is_match_all()),
            measure_id,
            metric: metric.filter(|metric| *metric != MetricName::value()),
        }
    }

    #[must_use]
    pub fn on_branch(self, branch_id: BranchId) -> Self {
        Self { branch_id, ..self }
    }

    #[must_use]
    pub fn metric_name(&self) -> MetricName {
        self.metric.clone().unwrap_or_else(MetricName::value)
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

    #[must_use]
    pub fn dimensions(&self) -> ThresholdDimensions {
        ThresholdDimensions {
            branch_id: self.branch_id,
            testbed_id: self.testbed_id,
            parameters: self.parameters.clone(),
            measure_id: self.measure_id,
            metric: self.metric.clone(),
        }
    }

    /// Whether this threshold checks every variant of the conventional `value` name.
    ///
    /// This is the only kind of threshold there was before a threshold could check
    /// anything narrower, so it is the one the deprecated singular fields carry
    /// wherever they appear: a row only a named or filtered threshold checks reports
    /// no check in them at all.
    #[must_use]
    pub fn is_bare(&self) -> bool {
        self.parameters.is_none() && self.metric.is_none()
    }

    pub fn find_by_dimensions(
        conn: &mut DbConnection,
        dimensions: &ThresholdDimensions,
    ) -> diesel::QueryResult<Option<Self>> {
        let mut query = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(dimensions.branch_id))
            .filter(schema::threshold::testbed_id.eq(dimensions.testbed_id))
            .filter(schema::threshold::measure_id.eq(dimensions.measure_id))
            .into_boxed();
        query = if let Some(parameters) = dimensions.parameters.clone() {
            query.filter(schema::threshold::parameters.eq(parameters))
        } else {
            query.filter(schema::threshold::parameters.is_null())
        };
        query = if let Some(metric) = dimensions.metric.clone() {
            query.filter(schema::threshold::metric.eq(metric))
        } else {
            query.filter(schema::threshold::metric.is_null())
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

    /// The model a slot holds.
    #[must_use]
    pub fn slot_model_id(&self, slot: ModelSlot) -> Option<ModelId> {
        match slot {
            ModelSlot::Model => self.model_id,
            ModelSlot::ChangepointModel => self.changepoint_model_id,
        }
    }

    /// Whether either slot holds a model, which is what the cap counts.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.model_id.is_some() || self.changepoint_model_id.is_some()
    }

    pub fn model(&self, conn: &mut DbConnection) -> Result<Option<QueryModel>, HttpError> {
        self.slot_model(conn, ModelSlot::Model)
    }

    fn slot_model(
        &self,
        conn: &mut DbConnection,
        slot: ModelSlot,
    ) -> Result<Option<QueryModel>, HttpError> {
        self.slot_model_id(slot)
            .map(|model_id| QueryModel::get(conn, model_id))
            .transpose()
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
                if current_model == model.with_defaults() {
                    ThresholdModelAction::NoChange
                } else {
                    ThresholdModelAction::Update(model)
                }
            },
        })
    }

    /// Fill, replace, or empty each slot the update names, unless it already holds the model.
    ///
    /// The cap and the rate limit are checked for every slot before any is written, and the
    /// slots are written in one transaction, so an update changes every slot it names or none.
    pub async fn update_slots_if_changed(
        &self,
        context: &ApiContext,
        models: ThresholdSlots<Option<Model>>,
    ) -> Result<(), HttpError> {
        let mut writes = Vec::new();
        for slot in ModelSlot::ALL {
            let Some(model) = models.get(slot) else {
                continue;
            };
            match Self::compute_model_action(auth_conn!(context), self.slot_model_id(slot), model)?
            {
                ThresholdModelAction::NoChange => {},
                action @ (ThresholdModelAction::Update(_) | ThresholdModelAction::Remove) => {
                    writes.push((slot, action));
                },
            }
        }
        let updates = writes
            .iter()
            .filter(|(_, action)| matches!(action, ThresholdModelAction::Update(_)))
            .count();
        if writes.is_empty() {
            return Ok(());
        }
        if updates > 0 && !self.is_active() {
            Self::check_active_cap(auth_conn!(context), &self.dimensions())?;
        }
        #[cfg(feature = "plus")]
        if let Some(count) = NonZeroU32::new(u32::try_from(updates).unwrap_or(u32::MAX)) {
            InsertModel::rate_limit_with_count(context, self, count).await?;
        }
        write_transaction!(context, |conn| {
            for (slot, action) in &writes {
                match action {
                    ThresholdModelAction::Update(model) => {
                        self.update_slot_inner(conn, *slot, *model)?;
                    },
                    ThresholdModelAction::Remove => self.remove_slot_model(conn, *slot)?,
                    ThresholdModelAction::NoChange => {},
                }
            }
            diesel::QueryResult::Ok(())
        })
        .map_err(|e| {
            crate::error::issue_error(
                "Failed to update threshold models",
                "Failed to update threshold models:",
                e,
            )
        })
    }

    /// What a report or a start point does to the slots of this threshold, each filled from
    /// `models`, and each that `models` leaves empty emptied only when `every_slot` holds.
    ///
    /// The new models of every slot are checked against the per threshold model ceiling together,
    /// as an update checks them. When they do not fit, the threshold is skipped once and keeps
    /// every slot `models` fills, while each slot it leaves empty is still emptied when
    /// `every_slot` holds.
    async fn slot_actions(
        &self,
        log: &Logger,
        context: &ApiContext,
        warnings: &mut ReportWarnings,
        models: &ThresholdSlots<Model>,
        every_slot: bool,
    ) -> Result<Vec<ThresholdAction>, HttpError> {
        let mut actions = Vec::new();
        for slot in ModelSlot::ALL {
            let model = models.get(slot);
            if model.is_none() && !every_slot {
                continue;
            }
            actions.push(
                match Self::compute_model_action(
                    auth_conn!(context),
                    self.slot_model_id(slot),
                    model,
                )? {
                    ThresholdModelAction::NoChange => ThresholdAction::NoChange,
                    ThresholdModelAction::Update(model) => {
                        ThresholdAction::Update(self.clone(), slot, model)
                    },
                    ThresholdModelAction::Remove => ThresholdAction::Remove(self.clone(), slot),
                },
            );
        }
        let updates = actions
            .iter()
            .filter(|action| matches!(action, ThresholdAction::Update(..)))
            .count();
        if !self.models_fit(log, context, updates).await? {
            warnings.skip(ReportWarningResource::Threshold);
            actions.retain(|action| !matches!(action, ThresholdAction::Update(..)));
        }
        Ok(actions)
    }

    /// Whether `count` more models fit under the per threshold model ceiling.
    async fn models_fit(
        &self,
        log: &Logger,
        context: &ApiContext,
        count: usize,
    ) -> Result<bool, HttpError> {
        #[cfg(feature = "plus")]
        if let Some(count) = NonZeroU32::new(u32::try_from(count).unwrap_or(u32::MAX))
            && past_the_ceiling(
                log,
                InsertModel::rate_limit_with_count(context, self, count).await,
            )?
            .is_none()
        {
            return Ok(false);
        }
        #[cfg(not(feature = "plus"))]
        let _ = (log, context, count);
        Ok(true)
    }

    fn update_slot_inner(
        &self,
        conn: &mut DbConnection,
        slot: ModelSlot,
        model: Model,
    ) -> diesel::QueryResult<()> {
        // Insert the new model
        let insert_model = InsertModel::new(self.id, model);
        diesel::insert_into(schema::model::table)
            .values(&insert_model)
            .execute(conn)?;

        // Get the new model ID and update the threshold
        let update_threshold = UpdateThreshold::new_model(conn, slot)?;
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(self.id)))
            .set(&update_threshold)
            .execute(conn)?;

        self.update_replaced_model(conn, slot, update_threshold.modified)
    }

    fn remove_slot_model(
        &self,
        conn: &mut DbConnection,
        slot: ModelSlot,
    ) -> diesel::QueryResult<()> {
        // Skip if the slot is empty
        if self.slot_model_id(slot).is_none() {
            return Ok(());
        }

        // Update the current threshold to empty the slot
        let update_threshold = UpdateThreshold::remove_model(slot);
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(self.id)))
            .set(&update_threshold)
            .execute(conn)?;

        self.update_replaced_model(conn, slot, update_threshold.modified)
    }

    fn remove_models(&self, conn: &mut DbConnection) -> diesel::QueryResult<()> {
        for slot in ModelSlot::ALL {
            self.remove_slot_model(conn, slot)?;
        }
        Ok(())
    }

    fn update_replaced_model(
        &self,
        conn: &mut DbConnection,
        slot: ModelSlot,
        date_time: DateTime,
    ) -> diesel::QueryResult<()> {
        // Update the old model to be replaced, if there is one
        if let Some(model_id) = self.slot_model_id(slot) {
            let update_model = UpdateModel::replaced_at(date_time);
            diesel::update(schema::model::table.filter(schema::model::id.eq(model_id)))
                .set(&update_model)
                .execute(conn)?;
            // A candidate judged against the replaced model is never confirmed, and no user saw it.
            #[cfg(feature = "plus")]
            if slot == ModelSlot::ChangepointModel {
                changepoint::QueryChangepoint::delete_candidates_of_threshold(conn, self.id)?;
            }
        }
        Ok(())
    }

    /// Refuse one more threshold with a model in either slot on a branch, testbed, and measure
    /// at the cap.
    fn check_active_cap(
        conn: &mut DbConnection,
        dimensions: &ThresholdDimensions,
    ) -> Result<(), HttpError> {
        let ThresholdDimensions {
            branch_id,
            testbed_id,
            measure_id,
            ..
        } = *dimensions;
        let active = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(branch_id))
            .filter(schema::threshold::testbed_id.eq(testbed_id))
            .filter(schema::threshold::measure_id.eq(measure_id))
            .filter(
                schema::threshold::model_id
                    .is_not_null()
                    .or(schema::threshold::changepoint_model_id.is_not_null()),
            )
            .count()
            .get_result::<i64>(conn)
            .map_err(resource_not_found_err!(
                Threshold,
                (branch_id, testbed_id, measure_id)
            ))?;
        if usize::try_from(active).is_ok_and(|active| active < MAX_ACTIVE_THRESHOLDS) {
            Ok(())
        } else {
            Err(bad_request_error(format!(
                "This branch, testbed, and measure already have {MAX_ACTIVE_THRESHOLDS} thresholds with a model or a changepoint model, which is the most they may have. Remove the models from one of them first."
            )))
        }
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
        let mut pinned = ThresholdSlots::default();
        pinned.set(query_model.test.slot(), query_model);
        query_threshold.into_json_for_models(
            conn,
            pinned,
            Some((head_id, version_id)),
            ThresholdSpec::Report(spec_id),
        )
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonThreshold, HttpError> {
        self.into_json_for_models(
            conn,
            ThresholdSlots::default(),
            None,
            ThresholdSpec::Testbed,
        )
    }

    /// The threshold with the model pinned in each slot, or else the slot's current model.
    pub fn into_json_for_models(
        self,
        conn: &mut DbConnection,
        pinned: ThresholdSlots<QueryModel>,
        head_version: Option<(HeadId, VersionId)>,
        threshold_spec: ThresholdSpec,
    ) -> Result<JsonThreshold, HttpError> {
        let model = self.slot_json(conn, ModelSlot::Model, pinned.model)?;
        let changepoint_model =
            self.slot_json(conn, ModelSlot::ChangepointModel, pinned.changepoint_model)?;
        let Self {
            uuid,
            project_id,
            branch_id,
            testbed_id,
            parameters,
            measure_id,
            metric,
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
            parameters,
            measure,
            metric,
            model,
            changepoint_model,
            created,
            modified,
        })
    }

    fn slot_json(
        &self,
        conn: &mut DbConnection,
        slot: ModelSlot,
        pinned: Option<QueryModel>,
    ) -> Result<Option<JsonModel>, HttpError> {
        let query_model = if pinned.is_some() {
            pinned
        } else {
            self.slot_model(conn, slot)?
        };
        Ok(query_model.map(|query_model| query_model.into_json(self)))
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
            parameters,
            metric,
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
            parameters,
            metric,
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
    pub parameters: Option<ParameterFilter>,
    pub measure_id: MeasureId,
    pub metric: Option<MetricName>,
    pub model_id: Option<ModelId>,
    pub changepoint_model_id: Option<ModelId>,
    pub created: DateTime,
    pub modified: DateTime,
}

/// The model each slot of a threshold holds, or is to hold.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThresholdSlots<T> {
    pub model: Option<T>,
    pub changepoint_model: Option<T>,
}

impl<T> Default for ThresholdSlots<T> {
    fn default() -> Self {
        Self {
            model: None,
            changepoint_model: None,
        }
    }
}

impl<T: Copy> ThresholdSlots<T> {
    #[must_use]
    pub fn get(&self, slot: ModelSlot) -> Option<T> {
        match slot {
            ModelSlot::Model => self.model,
            ModelSlot::ChangepointModel => self.changepoint_model,
        }
    }

    pub fn set(&mut self, slot: ModelSlot, value: T) {
        match slot {
            ModelSlot::Model => self.model = Some(value),
            ModelSlot::ChangepointModel => self.changepoint_model = Some(value),
        }
    }

    fn clear(&mut self, slot: ModelSlot) {
        match slot {
            ModelSlot::Model => self.model = None,
            ModelSlot::ChangepointModel => self.changepoint_model = None,
        }
    }

    fn is_empty(&self) -> bool {
        self.model.is_none() && self.changepoint_model.is_none()
    }
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

/// A write that a report or a start point plans for the thresholds of a branch.
enum ThresholdAction {
    Create(ThresholdDimensions, ThresholdSlots<Model>),
    Update(QueryThreshold, ModelSlot, Model),
    Remove(QueryThreshold, ModelSlot),
    NoChange,
}

impl InsertThreshold {
    #[cfg(feature = "plus")]
    crate::macros::rate_limit::fn_rate_limit!(threshold, Threshold);

    pub fn new(project_id: ProjectId, dimensions: ThresholdDimensions) -> Self {
        let timestamp = DateTime::now();
        let ThresholdDimensions {
            branch_id,
            testbed_id,
            parameters,
            measure_id,
            metric,
        } = dimensions;
        Self {
            uuid: ThresholdUuid::new(),
            project_id,
            branch_id,
            testbed_id,
            parameters,
            measure_id,
            metric,
            model_id: None,
            changepoint_model_id: None,
            created: timestamp,
            modified: timestamp,
        }
    }

    pub async fn from_models(
        context: &ApiContext,
        project_id: ProjectId,
        dimensions: ThresholdDimensions,
        models: ThresholdSlots<Model>,
    ) -> Result<ThresholdId, HttpError> {
        Self::check_unique(auth_conn!(context), &dimensions)?;

        if !models.is_empty() {
            QueryThreshold::check_active_cap(auth_conn!(context), &dimensions)?;
        }
        #[cfg(feature = "plus")]
        Self::rate_limit(context, project_id).await?;
        let threshold_id = write_transaction!(context, |conn| {
            Self::from_models_inner(conn, project_id, dimensions.clone(), models)
        })
        .map_err(|e| {
            crate::error::issue_error(
                "Failed to create threshold from model",
                "Failed to create threshold from model:",
                e,
            )
        })?;
        if let Some(threshold_id) = threshold_id {
            return Ok(threshold_id);
        }
        Self::check_unique(auth_conn!(context), &dimensions)?;
        Err(crate::error::issue_error(
            "Failed to create threshold from model",
            "Failed to create threshold from model:",
            "the threshold insert conflicted, but no threshold has its dimensions",
        ))
    }

    /// Refuse dimensions a threshold already holds.
    fn check_unique(
        conn: &mut DbConnection,
        dimensions: &ThresholdDimensions,
    ) -> Result<(), HttpError> {
        let Some(existing) = QueryThreshold::find_by_dimensions(conn, dimensions).map_err(|e| {
            crate::error::issue_error(
                "Failed to query threshold dimensions",
                "Failed to query threshold dimensions:",
                e,
            )
        })?
        else {
            return Ok(());
        };
        let metric = dimensions.metric_name();
        let parameters = dimensions
            .parameters
            .as_ref()
            .map_or_else(|| "every variant".to_owned(), ParameterFilter::canonical);
        Err(resource_conflict_error(
            BencherResource::Threshold,
            (
                dimensions.branch_id,
                dimensions.testbed_id,
                dimensions.measure_id,
            ),
            format!(
                "A threshold ({uuid}) already exists for this branch, testbed, and measure, checking {metric} of {parameters}",
                uuid = existing.uuid
            ),
        ))
    }

    fn from_models_inner(
        conn: &mut DbConnection,
        project_id: ProjectId,
        dimensions: ThresholdDimensions,
        models: ThresholdSlots<Model>,
    ) -> diesel::QueryResult<Option<ThresholdId>> {
        // Create the new threshold
        let insert_threshold = Self::new(project_id, dimensions);
        let inserted = diesel::insert_into(schema::threshold::table)
            .values(&insert_threshold)
            .on_conflict_do_nothing()
            .execute(conn)?;
        // A skipped insert leaves `last_insert_rowid()` at the last row this connection inserted.
        if inserted == 0 {
            return Ok(None);
        }

        // Get the new threshold ID
        let threshold_id = diesel::select(last_insert_rowid()).get_result::<ThresholdId>(conn)?;

        for slot in ModelSlot::ALL {
            let Some(model) = models.get(slot) else {
                continue;
            };
            // Create the new model
            let insert_model = InsertModel::new(threshold_id, model);
            diesel::insert_into(schema::model::table)
                .values(&insert_model)
                .execute(conn)?;

            // Get the new model ID and set it in its slot of the threshold
            let model_id = diesel::select(last_insert_rowid()).get_result::<ModelId>(conn)?;
            let threshold = diesel::update(
                schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)),
            );
            match slot {
                ModelSlot::Model => threshold
                    .set(schema::threshold::model_id.eq(model_id))
                    .execute(conn)?,
                ModelSlot::ChangepointModel => threshold
                    .set(schema::threshold::changepoint_model_id.eq(model_id))
                    .execute(conn)?,
            };
        }

        Ok(Some(threshold_id))
    }

    pub async fn from_start_point(
        log: &Logger,
        context: &ApiContext,
        query_branch: &QueryBranch,
        branch_start_point: &StartPoint,
        warnings: &mut ReportWarnings,
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
        let (actions, orphans) = Self::compute_start_point_actions(
            log,
            context,
            query_branch,
            branch_start_point,
            warnings,
        )
        .await?;

        // Phase 2: Batch all writes in a single write lock + transaction.
        let has_writes = actions
            .iter()
            .any(|a| !matches!(a, ThresholdAction::NoChange))
            || !orphans.is_empty();
        if has_writes {
            let project_id = query_branch.project_id;
            write_transaction!(context, |conn| {
                Self::apply_actions(log, conn, project_id, actions, orphans)
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
        warnings: &mut ReportWarnings,
    ) -> Result<(Vec<ThresholdAction>, Vec<QueryThreshold>), HttpError> {
        let mut current_thresholds = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(query_branch.id))
            .load::<QueryThreshold>(auth_conn!(context))
            .map_err(resource_not_found_err!(
                Threshold,
                &branch_start_point.branch
            ))?
            .into_iter()
            .map(|threshold| (threshold.dimensions(), threshold))
            .collect::<HashMap<_, _>>();
        slog::debug!(log, "Current thresholds: {current_thresholds:?}");
        let mut active = ActiveThresholds::new(&current_thresholds);

        // Fetch start point thresholds with their models in a single JOIN query, in the UUID
        // order that decides which of them fit under the cap
        let start_point_thresholds = schema::threshold::table
            .left_join(
                schema::model::table
                    .on(schema::model::id.nullable().eq(schema::threshold::model_id)),
            )
            .filter(schema::threshold::branch_id.eq(branch_start_point.branch.id))
            .order(schema::threshold::uuid.asc())
            .select((
                QueryThreshold::as_select(),
                Option::<QueryModel>::as_select(),
            ))
            .load::<(QueryThreshold, Option<QueryModel>)>(auth_conn!(context))
            .map_err(resource_not_found_err!(
                Threshold,
                &branch_start_point.branch
            ))?;
        let changepoint_model_ids = start_point_thresholds
            .iter()
            .filter_map(|(threshold, _)| threshold.changepoint_model_id)
            .collect::<Vec<_>>();
        let changepoint_models = schema::model::table
            .filter(schema::model::id.eq_any(&changepoint_model_ids))
            .select(QueryModel::as_select())
            .load::<QueryModel>(auth_conn!(context))
            .map_err(resource_not_found_err!(Model, &changepoint_model_ids))?
            .into_iter()
            .map(|model| (model.id, model.into_model()))
            .collect::<HashMap<_, _>>();
        let start_point_thresholds = start_point_thresholds
            .into_iter()
            .map(|(threshold, model)| {
                let models = ThresholdSlots {
                    model: model.map(QueryModel::into_model),
                    changepoint_model: threshold
                        .changepoint_model_id
                        .and_then(|model_id| changepoint_models.get(&model_id).copied()),
                };
                (threshold.dimensions().on_branch(query_branch.id), models)
            })
            .collect::<Vec<_>>();
        slog::debug!(log, "Start point thresholds: {start_point_thresholds:?}");

        // Pre-compute actions using read connections
        let mut actions = Vec::new();
        for (start_point_dimensions, start_point_models) in start_point_thresholds {
            if let Some(current_threshold) = current_thresholds.remove(&start_point_dimensions) {
                for action in current_threshold
                    .slot_actions(log, context, warnings, &start_point_models, true)
                    .await?
                {
                    active.release_removed(&action);
                    actions.push(action);
                }
            } else if start_point_models.is_empty() {
                actions.push(ThresholdAction::NoChange);
            } else {
                actions.push(ThresholdAction::Create(
                    start_point_dimensions,
                    start_point_models,
                ));
            }
        }

        // Remaining current thresholds are orphans to remove
        let orphans: Vec<QueryThreshold> = current_thresholds.into_values().collect();
        slog::debug!(log, "Orphan thresholds to remove: {orphans:?}");
        for orphan in &orphans {
            active.release(orphan);
        }

        actions.retain(|action| active.admits(log, action));
        let admitted = actions.len();
        #[cfg(feature = "plus")]
        {
            let creates = actions
                .iter()
                .filter(|action| matches!(action, ThresholdAction::Create(..)))
                .count();
            if !Self::ceiling_admits(log, context, query_branch.project_id, creates).await? {
                actions.retain(|action| !matches!(action, ThresholdAction::Create(..)));
            }
        }
        warnings.skip_with_count(
            ReportWarningResource::Threshold,
            active.skipped + admitted - actions.len(),
        );

        Ok((actions, orphans))
    }

    /// The thresholds a report declares, created or updated for its branch and its
    /// testbed.
    pub async fn from_report_json(
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        branch_id: BranchId,
        testbed_id: TestbedId,
        json_thresholds: Option<JsonReportThresholds>,
        warnings: &mut ReportWarnings,
    ) -> Result<(), HttpError> {
        let Some(json_thresholds) = json_thresholds else {
            slog::debug!(log, "No thresholds in report");
            return Ok(());
        };
        let JsonReportThresholds { models, reset } = json_thresholds;

        let reset_thresholds = reset.unwrap_or_default();
        if models
            .as_ref()
            .is_none_or(JsonReportThresholdModels::is_empty)
            && !reset_thresholds
        {
            slog::debug!(log, "No threshold models or reset in report");
            return Ok(());
        }

        let mut current_thresholds = schema::threshold::table
            .filter(schema::threshold::project_id.eq(project_id))
            .filter(schema::threshold::branch_id.eq(branch_id))
            .filter(schema::threshold::testbed_id.eq(testbed_id))
            .load::<QueryThreshold>(auth_conn!(context))
            .map_err(resource_not_found_err!(Threshold, (branch_id, testbed_id)))?
            .into_iter()
            .map(|threshold| (threshold.dimensions(), threshold))
            .collect::<HashMap<_, _>>();
        slog::debug!(log, "Current thresholds: {current_thresholds:?}");
        let mut active = ActiveThresholds::new(&current_thresholds);

        // Phase 1: Pre-resolve all measure IDs (may trigger get_or_create writes)
        // and read current model state.
        let declared = Self::declared_thresholds(
            log, context, project_id, branch_id, testbed_id, models, warnings,
        )
        .await?;
        let mut actions = Vec::new();
        for (dimensions, models) in declared {
            let measure_id = dimensions.measure_id;
            slog::debug!(log, "Processing threshold for measure {measure_id}");
            if let Some(current_threshold) = current_thresholds.remove(&dimensions) {
                // Only a reset empties a slot the report does not name.
                for action in current_threshold
                    .slot_actions(log, context, warnings, &models, reset_thresholds)
                    .await?
                {
                    active.release_removed(&action);
                    actions.push(action);
                }
            } else {
                slog::debug!(log, "Creating threshold for measure {measure_id}");
                actions.push(ThresholdAction::Create(dimensions, models));
            }
        }

        // Collect orphan thresholds to reset
        let orphans: Vec<QueryThreshold> = if reset_thresholds {
            current_thresholds.into_values().collect()
        } else {
            Vec::new()
        };
        for orphan in &orphans {
            active.release(orphan);
        }

        // Past the cap or the ceiling, a report skips what does not fit and keeps the rest.
        actions.retain(|action| active.admits(log, action));
        let admitted = actions.len();
        #[cfg(feature = "plus")]
        {
            let creates = actions
                .iter()
                .filter(|action| matches!(action, ThresholdAction::Create(..)))
                .count();
            if !Self::ceiling_admits(log, context, project_id, creates).await? {
                actions.retain(|action| !matches!(action, ThresholdAction::Create(..)));
            }
        }
        warnings.skip_with_count(
            ReportWarningResource::Threshold,
            active.skipped + admitted - actions.len(),
        );

        // Phase 2: Batch all threshold writes in a single write lock acquisition
        // wrapped in a transaction for atomicity.
        // Skip if there's nothing to write.
        let has_writes = actions
            .iter()
            .any(|a| !matches!(a, ThresholdAction::NoChange))
            || !orphans.is_empty();
        if has_writes {
            write_transaction!(context, |conn| {
                Self::apply_actions(log, conn, project_id, actions, orphans)
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

    /// The thresholds the payload declares, each resolved to the dimensions it
    /// addresses, skipping one whose measure a daily limit refused or whose changepoint model
    /// the report refused as it read it.
    async fn declared_thresholds(
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        branch_id: BranchId,
        testbed_id: TestbedId,
        models: Option<JsonReportThresholdModels>,
        warnings: &mut ReportWarnings,
    ) -> Result<Vec<(ThresholdDimensions, ThresholdSlots<Model>)>, HttpError> {
        let mut declared = Vec::new();
        let mut positions = HashMap::new();
        match models {
            None => {},
            Some(JsonReportThresholdModels::Map(models)) => {
                for (measure, model) in models {
                    let Some(measure_id) = past_the_ceiling(
                        log,
                        QueryMeasure::get_or_create(context, project_id, &measure).await,
                    )?
                    else {
                        warnings.skip(ReportWarningResource::Threshold);
                        continue;
                    };
                    Self::declare(
                        &mut declared,
                        &mut positions,
                        ThresholdDimensions::new(branch_id, testbed_id, None, measure_id, None),
                        declared_model(log, warnings, model),
                    );
                }
            },
            Some(JsonReportThresholdModels::List(entries)) => {
                for entry in entries {
                    let JsonReportThresholdEntry {
                        parameters,
                        measure,
                        metric,
                        model,
                    } = entry;
                    let model = match model {
                        JsonReportThresholdEntryModel::Model(model) => model,
                        // Ingest is lenient, so the report goes on without the entry.
                        JsonReportThresholdEntryModel::Refused { error, .. } => {
                            slog::warn!(
                                log,
                                "Skipping the changepoint model of measure {measure}, metric {metric}: {error}"
                            );
                            warnings.skip(ReportWarningResource::ChangepointModel);
                            continue;
                        },
                    };
                    let Some(measure_id) = past_the_ceiling(
                        log,
                        QueryMeasure::get_or_create(context, project_id, &measure).await,
                    )?
                    else {
                        warnings.skip(ReportWarningResource::Threshold);
                        continue;
                    };
                    Self::declare(
                        &mut declared,
                        &mut positions,
                        ThresholdDimensions::new(
                            branch_id,
                            testbed_id,
                            parameters,
                            measure_id,
                            Some(metric),
                        ),
                        declared_model(log, warnings, model),
                    );
                }
            },
        }
        Ok(declared)
    }

    /// Dimensions declared twice are one threshold, at the first position with the last model
    /// in each slot. The slot of a model follows from its test.
    fn declare(
        declared: &mut Vec<(ThresholdDimensions, ThresholdSlots<Model>)>,
        positions: &mut HashMap<ThresholdDimensions, usize>,
        dimensions: ThresholdDimensions,
        model: Model,
    ) {
        let slot = model.test.slot();
        match positions.entry(dimensions) {
            Entry::Occupied(position) => {
                if let Some((_, declared_models)) = declared.get_mut(*position.get()) {
                    declared_models.set(slot, model);
                }
            },
            Entry::Vacant(position) => {
                let dimensions = position.key().clone();
                position.insert(declared.len());
                let mut models = ThresholdSlots::default();
                models.set(slot, model);
                declared.push((dimensions, models));
            },
        }
    }

    /// Phase 2 of a report or a start point sync: apply the pre-computed actions.
    fn apply_actions(
        log: &Logger,
        conn: &mut DbConnection,
        project_id: ProjectId,
        actions: Vec<ThresholdAction>,
        orphans: Vec<QueryThreshold>,
    ) -> diesel::QueryResult<()> {
        for action in actions {
            match action {
                ThresholdAction::Create(dimensions, models) => {
                    if Self::from_models_inner(conn, project_id, dimensions, models)?.is_none() {
                        slog::debug!(log, "Skipped a threshold another request created first");
                    }
                },
                ThresholdAction::Update(threshold, slot, model) => {
                    threshold.update_slot_inner(conn, slot, model)?;
                },
                ThresholdAction::Remove(threshold, slot) => {
                    threshold.remove_slot_model(conn, slot)?;
                },
                ThresholdAction::NoChange => {},
            }
        }
        for threshold in orphans {
            threshold.remove_models(conn)?;
            slog::debug!(log, "Removed the models from threshold {threshold:?}");
        }
        Ok(())
    }

    /// Only the ceiling's own 429 is a `false`; every other error propagates.
    #[cfg(feature = "plus")]
    async fn ceiling_admits(
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        creates: usize,
    ) -> Result<bool, HttpError> {
        let Some(count) = NonZeroU32::new(u32::try_from(creates).unwrap_or(u32::MAX)) else {
            return Ok(true);
        };
        match Self::rate_limit_with_count(context, project_id, count).await {
            Ok(()) => Ok(true),
            Err(error) if error.status_code == ClientErrorStatusCode::TOO_MANY_REQUESTS => {
                slog::warn!(
                    log,
                    "Skipping {creates} threshold creation(s) past the daily ceiling: {error}"
                );
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment_by(
                    bencher_otel::ApiCounter::ThresholdCeilingSkip,
                    u64::try_from(creates).unwrap_or(u64::MAX),
                );
                Ok(false)
            },
            Err(error) => Err(error),
        }
    }
}

/// A report's model, with a warning for each field the report set aside because its test does not
/// take it, as it ignored the field before a model had one.
fn declared_model(log: &Logger, warnings: &mut ReportWarnings, model: JsonReportModel) -> Model {
    let JsonReportModel {
        model,
        ignored_significance,
        ignored_confirmations,
    } = model;
    for (ignored, field, resource) in [
        (
            ignored_significance,
            "significance",
            ReportWarningResource::Significance,
        ),
        (
            ignored_confirmations,
            "confirmations",
            ReportWarningResource::Confirmations,
        ),
    ] {
        if ignored {
            slog::warn!(
                log,
                "Ignoring the {field} of a report's {} model",
                model.test
            );
            warnings.ignore_with_count(resource, 1);
        }
    }
    model
}

/// How many thresholds with a model in either slot each branch, testbed, and measure has, for
/// the cap.
struct ActiveThresholds {
    counts: HashMap<(BranchId, TestbedId, MeasureId), usize>,
    /// The slots of each threshold that a planned write touches, as they are once it is done.
    planned: HashMap<ThresholdId, ThresholdSlots<()>>,
    /// The thresholds past the cap, whose every planned write is refused.
    refused: HashSet<ThresholdId>,
    /// How many thresholds a planned write was refused for, each counted once.
    skipped: usize,
}

impl ActiveThresholds {
    fn new(thresholds: &HashMap<ThresholdDimensions, QueryThreshold>) -> Self {
        let mut counts = HashMap::new();
        for threshold in thresholds
            .values()
            .filter(|threshold| threshold.is_active())
        {
            *counts
                .entry((
                    threshold.branch_id,
                    threshold.testbed_id,
                    threshold.measure_id,
                ))
                .or_default() += 1;
        }
        Self {
            counts,
            planned: HashMap::new(),
            refused: HashSet::new(),
            skipped: 0,
        }
    }

    /// Whether a planned write fits under the cap, counting it when it gives a threshold its
    /// first model.
    fn admits(&mut self, log: &Logger, action: &ThresholdAction) -> bool {
        let admitted = match action {
            ThresholdAction::Create(dimensions, models) => {
                models.is_empty() || self.admit(log, dimensions)
            },
            ThresholdAction::Update(threshold, slot, _) => {
                // A threshold refused for one slot is refused for the other, and skipped once.
                if self.refused.contains(&threshold.id) {
                    return false;
                }
                if self.planned_slots(threshold).is_empty()
                    && !self.admit(log, &threshold.dimensions())
                {
                    self.refused.insert(threshold.id);
                    false
                } else {
                    self.planned_slots(threshold).set(*slot, ());
                    true
                }
            },
            ThresholdAction::Remove(..) | ThresholdAction::NoChange => true,
        };
        if !admitted {
            self.skipped += 1;
        }
        admitted
    }

    fn admit(&mut self, log: &Logger, dimensions: &ThresholdDimensions) -> bool {
        let active = self
            .counts
            .entry((
                dimensions.branch_id,
                dimensions.testbed_id,
                dimensions.measure_id,
            ))
            .or_default();
        if *active < MAX_ACTIVE_THRESHOLDS {
            *active += 1;
            true
        } else {
            slog::warn!(
                log,
                "Skipping a threshold past the cap of {MAX_ACTIVE_THRESHOLDS} with a model on its branch, testbed, and measure: {dimensions:?}"
            );
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ThresholdCapSkip);
            false
        }
    }

    /// Release a threshold whose planned write empties its last filled slot.
    fn release_removed(&mut self, action: &ThresholdAction) {
        if let ThresholdAction::Remove(threshold, slot) = action {
            let slots = self.planned_slots(threshold);
            let active = !slots.is_empty();
            slots.clear(*slot);
            if active && slots.is_empty() {
                self.release_dimensions(threshold);
            }
        }
    }

    /// Release a threshold whose every slot is to be emptied.
    fn release(&mut self, threshold: &QueryThreshold) {
        let slots = self.planned_slots(threshold);
        let active = !slots.is_empty();
        *slots = ThresholdSlots::default();
        if active {
            self.release_dimensions(threshold);
        }
    }

    fn release_dimensions(&mut self, threshold: &QueryThreshold) {
        if let Some(active) = self.counts.get_mut(&(
            threshold.branch_id,
            threshold.testbed_id,
            threshold.measure_id,
        )) {
            *active = active.saturating_sub(1);
        }
    }

    fn planned_slots(&mut self, threshold: &QueryThreshold) -> &mut ThresholdSlots<()> {
        self.planned
            .entry(threshold.id)
            .or_insert_with(|| ThresholdSlots {
                model: threshold.model_id.map(|_| ()),
                changepoint_model: threshold.changepoint_model_id.map(|_| ()),
            })
    }
}

/// Refuse a report whose thresholds are not in the shape its BMF version, declared
/// or taken from the project, spells.
///
/// At BMF version 0 a map key names a measure and nothing else, so its model can only be the
/// bare threshold's `model`, and a model whose test checks a line is refused.
pub fn check_report_thresholds_shape(
    bmf_version: BmfVersion,
    json_thresholds: Option<&JsonReportThresholds>,
) -> Result<(), HttpError> {
    let Some(models) = json_thresholds.and_then(|thresholds| thresholds.models.as_ref()) else {
        return Ok(());
    };
    let (expected, sent) = match bmf_version {
        BmfVersion::V0 => match models {
            JsonReportThresholdModels::Map(models) => {
                return models
                    .values()
                    .try_for_each(|model| ModelSlot::Model.admit(model.model.test))
                    .map_err(|e| {
                        bad_request_error(format!(
                            "The report is read as BMF version {bmf_version}, so `thresholds.models` holds only models that check a metric: {e}"
                        ))
                    });
            },
            JsonReportThresholdModels::List(_) => ("a map of measure to threshold model", "a list"),
        },
        _ => match models {
            JsonReportThresholdModels::Map(_) => ("a list of threshold entries", "a map"),
            JsonReportThresholdModels::List(_) => return Ok(()),
        },
    };
    Err(bad_request_error(format!(
        "The report is read as BMF version {bmf_version}, so `thresholds.models` must be {expected}, but {sent} was sent."
    )))
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = threshold_table)]
pub struct UpdateThreshold {
    pub model_id: Option<Option<ModelId>>,
    pub changepoint_model_id: Option<Option<ModelId>>,
    pub modified: DateTime,
}

impl UpdateThreshold {
    /// Create an `UpdateThreshold` that sets a slot to the most recently inserted model.
    ///
    /// # Precondition
    /// Must be called immediately after an `INSERT INTO model` on the same connection,
    /// within the same transaction. Uses `last_insert_rowid()` to retrieve the model ID.
    pub fn new_model(conn: &mut DbConnection, slot: ModelSlot) -> diesel::QueryResult<Self> {
        let model_id = diesel::select(last_insert_rowid()).get_result::<ModelId>(conn)?;
        Ok(Self::slot(slot, Some(model_id)))
    }

    pub fn remove_model(slot: ModelSlot) -> Self {
        Self::slot(slot, None)
    }

    fn slot(slot: ModelSlot, model_id: Option<ModelId>) -> Self {
        let (model_id, changepoint_model_id) = match slot {
            ModelSlot::Model => (Some(model_id), None),
            ModelSlot::ChangepointModel => (None, Some(model_id)),
        };
        Self {
            model_id,
            changepoint_model_id,
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
            create_base_entities, create_branch_with_head, create_measure, create_model,
            create_testbed, create_threshold, create_threshold_with_dimensions,
            get_threshold_model_id, get_thresholds_for_branch, setup_test_db,
        },
    };

    use super::{
        ActiveThresholds, BranchId, DbConnection, InsertThreshold, MAX_ACTIVE_THRESHOLDS,
        MetricName, Model, ModelSlot, ParameterFilter, QueryThreshold, ThresholdAction,
        ThresholdDimensions, ThresholdId, ThresholdSlots, UpdateThreshold, model::ModelId,
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
    /// The `UpdateThreshold::remove_model(ModelSlot::Model)` sets `model_id` to `None`.
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
        let update_threshold = UpdateThreshold::remove_model(ModelSlot::Model);
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
            &ThresholdDimensions::new(branch.branch_id, testbed, None, measure, None),
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
            &ThresholdDimensions::new(branch.branch_id, testbed, None, measure, None),
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

    /// The dimensions every threshold in these tests hangs off.
    fn dimensions_fixture(conn: &mut DbConnection) -> (ProjectId, BranchId, TestbedId, MeasureId) {
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
        (base.project_id, branch.branch_id, testbed, measure)
    }

    /// The conventional `value` name spelled out is the same threshold as no name at
    /// all, and the canonical form of both is no name at all.
    #[test]
    fn dimensions_collapse_the_value_name() {
        let mut conn = setup_test_db();
        let (_project_id, branch_id, testbed_id, measure_id) = dimensions_fixture(&mut conn);

        let bare = ThresholdDimensions::new(branch_id, testbed_id, None, measure_id, None);
        let explicit = ThresholdDimensions::new(
            branch_id,
            testbed_id,
            None,
            measure_id,
            Some(metric_name("value")),
        );
        assert_eq!(bare, explicit);
        assert!(explicit.metric.is_none());
        assert_eq!(explicit.metric_name(), MetricName::value());

        let named = ThresholdDimensions::new(
            branch_id,
            testbed_id,
            None,
            measure_id,
            Some(metric_name("p99")),
        );
        assert_ne!(bare, named);
        assert_eq!(named.metric_name(), metric_name("p99"));
    }

    /// A filter that matches every variant is the same threshold as no filter, in
    /// either of the two spellings that say so.
    #[test]
    fn dimensions_collapse_a_match_all_filter() {
        let mut conn = setup_test_db();
        let (_project_id, branch_id, testbed_id, measure_id) = dimensions_fixture(&mut conn);
        let on = |parameters| {
            ThresholdDimensions::new(branch_id, testbed_id, parameters, measure_id, None)
        };

        let bare = on(None);
        assert_eq!(bare, on(Some(filter("[]"))));
        assert_eq!(bare, on(Some(filter("[{}]"))));
        // A filter holding the empty set matches everything the rest of it could
        // have narrowed, so it is match all too.
        assert_eq!(bare, on(Some(filter(r#"[{"size": 512}, {}]"#))));

        let filtered = on(Some(filter(r#"[{"size": 512}]"#)));
        assert_ne!(bare, filtered);
        assert!(filtered.parameters.is_some());
    }

    /// Two thresholds that check the same name of the same variants collide,
    /// whichever way each spells it.
    #[test]
    fn duplicate_dimensions_insert_fails_at_both_spellings() {
        let mut conn = setup_test_db();
        let (project_id, branch_id, testbed_id, measure_id) = dimensions_fixture(&mut conn);

        // A bare threshold and one that spells out everything it defaults to.
        create_threshold(
            &mut conn,
            project_id,
            branch_id,
            testbed_id,
            measure_id,
            "00000000-0000-0000-0000-000000000040",
        );
        let spelled_out = ThresholdDimensions::new(
            branch_id,
            testbed_id,
            Some(filter("[{}]")),
            measure_id,
            Some(metric_name("value")),
        );
        // The canonical form of the spelled out dimensions is the bare one, which is
        // what makes the two collide.
        assert_eq!(
            spelled_out,
            ThresholdDimensions::new(branch_id, testbed_id, None, measure_id, None)
        );
        let insert = InsertThreshold::new(project_id, spelled_out);
        diesel::insert_into(schema::threshold::table)
            .values(&insert)
            .execute(&mut conn)
            .unwrap_err();

        // A named and filtered threshold sits beside the bare one.
        let filtered = ThresholdDimensions::new(
            branch_id,
            testbed_id,
            Some(filter(r#"[{"size": 512}]"#)),
            measure_id,
            Some(metric_name("p99")),
        );
        let insert = InsertThreshold::new(project_id, filtered.clone());
        diesel::insert_into(schema::threshold::table)
            .values(&insert)
            .execute(&mut conn)
            .expect("A filtered threshold sits beside the bare one");

        // But a second one that spells the same filter differently does not.
        let restated = ThresholdDimensions::new(
            branch_id,
            testbed_id,
            Some(filter(r#"[{"size": 512.0}, {"size": 512}]"#)),
            measure_id,
            Some(metric_name("p99")),
        );
        assert_eq!(filtered, restated);
        let insert = InsertThreshold::new(project_id, restated);
        diesel::insert_into(schema::threshold::table)
            .values(&insert)
            .execute(&mut conn)
            .unwrap_err();
    }

    /// A stored filter round trips through the column byte for byte.
    #[test]
    fn filter_round_trips_through_the_column() {
        let mut conn = setup_test_db();
        let (project_id, branch_id, testbed_id, measure_id) = dimensions_fixture(&mut conn);

        let stored = filter(r#"[{"threads": 4, "size": 1024}, {"size": 512}]"#);
        create_threshold_with_dimensions(
            &mut conn,
            project_id,
            branch_id,
            testbed_id,
            Some(stored.clone()),
            measure_id,
            Some(metric_name("p99")),
            "00000000-0000-0000-0000-000000000040",
        );

        let read = schema::threshold::table
            .select(QueryThreshold::as_select())
            .first::<QueryThreshold>(&mut conn)
            .expect("Failed to read the threshold");
        assert_eq!(read.metric, Some(metric_name("p99")));
        assert_eq!(read.parameters, Some(stored));
        // Canonical bytes, not numeric value: `{"size":1` sorts before `{"size":5`.
        assert_eq!(
            read.dimensions().parameters.expect("No filter").canonical(),
            r#"[{"size":1024,"threads":4},{"size":512}]"#
        );
    }

    #[derive(diesel::QueryableByName)]
    struct SqlText {
        #[diesel(sql_type = diesel::sql_types::Text)]
        value: String,
    }

    fn sql_text(conn: &mut DbConnection, sql: &str, blob: Vec<u8>) -> String {
        diesel::sql_query(format!("SELECT {sql} AS value"))
            .bind::<diesel::sql_types::Binary, _>(blob)
            .get_result::<SqlText>(conn)
            .expect("Failed to read the encoded filter")
            .value
    }

    /// The filter encoder has to be byte identical to `SQLite`'s own `jsonb()` over
    /// the same canonical text, because the unique index compares the stored bytes
    /// and `SQLite`'s JSON functions read them back.
    #[test]
    fn filter_bytes_agree_with_sqlite_jsonb() {
        let mut conn = setup_test_db();
        // The empty filter, one set, several sets, and a filter whose payload is past
        // the one byte size a JSONB header carries after the inline nibble.
        let long = "x".repeat(64);
        let wide = ["a", "b", "c", "d"]
            .map(|key| format!(r#"{{"{key}":"{long}"}}"#))
            .join(",");
        for canonical in [
            "[]".to_owned(),
            r#"[{"size":512}]"#.to_owned(),
            r#"[{"size":1024,"threads":4},{"size":512}]"#.to_owned(),
            format!("[{wide}]"),
        ] {
            let parsed = filter(&canonical);
            assert_eq!(
                parsed.canonical(),
                canonical,
                "{canonical}: the text is already canonical"
            );
            let blob = parsed.to_jsonb().expect("Failed to encode the filter");
            assert_eq!(
                sql_text(&mut conn, "hex(?)", blob.clone()),
                sql_text(
                    &mut conn,
                    "hex(jsonb(CAST(? AS TEXT)))",
                    canonical.clone().into_bytes()
                ),
                "{canonical}: the encoded bytes must be the bytes jsonb() mints"
            );
            assert_eq!(
                sql_text(&mut conn, "json(?)", blob),
                canonical,
                "{canonical}: the canonical text must survive the encoder unchanged"
            );
        }
    }

    /// A create on dimensions a threshold already holds is skipped, and the held
    /// threshold keeps its model.
    #[test]
    fn a_create_on_held_dimensions_is_skipped() {
        let mut conn = setup_test_db();
        let (project_id, bare, _named) = two_dimensions(&mut conn);
        let (held, held_model) = create_held(
            &mut conn,
            project_id,
            &bare,
            "00000000-0000-0000-0000-000000000040",
            "00000000-0000-0000-0000-000000000050",
        );

        let created = InsertThreshold::from_models_inner(
            &mut conn,
            project_id,
            bare.clone(),
            metric_model(Model::upper_boundary()),
        )
        .expect("A held create must not fail");

        assert_eq!(created, None);
        assert_eq!(
            get_thresholds_for_branch(&mut conn, bare.branch_id),
            vec![held]
        );
        assert_eq!(
            models_of(&mut conn, held),
            (Some(held_model), vec![held_model])
        );
        assert_eq!(
            schema::model::table
                .count()
                .get_result::<i64>(&mut conn)
                .expect("Failed to count models"),
            1
        );
    }

    /// A report or a start point sync skips a create whose dimensions another request took
    /// first and commits the rest of its batch.
    #[test]
    fn a_sync_skips_a_held_create_and_commits_the_rest() {
        let mut conn = setup_test_db();
        let (project_id, bare, named) = two_dimensions(&mut conn);
        let (held, held_model) = create_held(
            &mut conn,
            project_id,
            &bare,
            "00000000-0000-0000-0000-000000000040",
            "00000000-0000-0000-0000-000000000050",
        );

        conn.immediate_transaction(|conn| {
            InsertThreshold::apply_actions(
                &discard(),
                conn,
                project_id,
                vec![
                    ThresholdAction::Create(bare, metric_model(Model::lower_boundary())),
                    ThresholdAction::Create(named.clone(), metric_model(Model::upper_boundary())),
                ],
                Vec::new(),
            )
        })
        .expect("A held create must not fail the batch");

        assert_eq!(
            models_of(&mut conn, held),
            (Some(held_model), vec![held_model])
        );
        assert_eq!(sole_model(&mut conn, &named), Model::upper_boundary());
    }

    fn metric_model(model: Model) -> ThresholdSlots<Model> {
        ThresholdSlots {
            model: Some(model),
            changepoint_model: None,
        }
    }

    fn discard() -> slog::Logger {
        slog::Logger::root(slog::Discard, slog::o!())
    }

    /// Bare dimensions, and named and filtered dimensions beside them on the same branch.
    fn two_dimensions(
        conn: &mut DbConnection,
    ) -> (ProjectId, ThresholdDimensions, ThresholdDimensions) {
        let (project_id, branch_id, testbed_id, measure_id) = dimensions_fixture(conn);
        let bare = ThresholdDimensions::new(branch_id, testbed_id, None, measure_id, None);
        let named = ThresholdDimensions::new(
            branch_id,
            testbed_id,
            Some(filter(r#"[{"size": 512}]"#)),
            measure_id,
            Some(metric_name("p99")),
        );
        (project_id, bare, named)
    }

    /// A threshold with a model, as another request would have created it.
    fn create_held(
        conn: &mut DbConnection,
        project_id: ProjectId,
        dimensions: &ThresholdDimensions,
        threshold_uuid: &str,
        model_uuid: &str,
    ) -> (ThresholdId, ModelId) {
        let ThresholdDimensions {
            branch_id,
            testbed_id,
            parameters,
            measure_id,
            metric,
        } = dimensions.clone();
        let threshold_id = create_threshold_with_dimensions(
            conn,
            project_id,
            branch_id,
            testbed_id,
            parameters,
            measure_id,
            metric,
            threshold_uuid,
        );
        let model_id = create_model(conn, threshold_id, model_uuid, 0);
        (threshold_id, model_id)
    }

    /// A threshold's current model and every model it has had.
    fn models_of(
        conn: &mut DbConnection,
        threshold_id: ThresholdId,
    ) -> (Option<ModelId>, Vec<ModelId>) {
        let models = schema::model::table
            .filter(schema::model::threshold_id.eq(threshold_id))
            .order(schema::model::id)
            .select(schema::model::id)
            .load::<ModelId>(conn)
            .expect("Failed to load the threshold's models");
        (get_threshold_model_id(conn, threshold_id), models)
    }

    /// The only model the threshold at these dimensions has ever had.
    fn sole_model(conn: &mut DbConnection, dimensions: &ThresholdDimensions) -> Model {
        let threshold = QueryThreshold::find_by_dimensions(conn, dimensions)
            .expect("Failed to find the threshold")
            .expect("No threshold has these dimensions");
        let (current, models) = models_of(conn, threshold.id);
        assert_eq!(current.map(|model_id| vec![model_id]), Some(models));
        threshold
            .model(conn)
            .expect("Failed to read the model")
            .expect("The threshold has no model")
            .into_model()
    }

    // A threshold with both slots filled counts once toward the cap, and so does a create of one.
    #[test]
    fn the_cap_counts_a_threshold_once_with_both_slots() {
        let mut slots = vec![BOTH; MAX_ACTIVE_THRESHOLDS - 2];
        slots.push(CHANGEPOINT);
        let current = thresholds_on_one_measure(&slots);
        let mut active = ActiveThresholds::new(&current);
        let log = discard();
        assert!(active.admits(&log, &create(BOTH)));
        assert!(!active.admits(&log, &create(BOTH)));
    }

    // A threshold with only a changepoint model counts toward the cap, and so does a create of one.
    #[test]
    fn the_cap_counts_a_threshold_with_only_a_changepoint_model() {
        let current = thresholds_on_one_measure(&[CHANGEPOINT; MAX_ACTIVE_THRESHOLDS - 1]);
        let mut active = ActiveThresholds::new(&current);
        let log = discard();
        assert!(active.admits(&log, &create(CHANGEPOINT)));
        assert!(!active.admits(&log, &create(CHANGEPOINT)));
        assert!(
            active.admits(&log, &create(EMPTY)),
            "a create with no model does not count"
        );
    }

    // Filling both slots of an empty threshold counts it once.
    #[test]
    fn the_cap_counts_both_slots_of_one_update_once() {
        let mut slots = vec![MODEL; MAX_ACTIVE_THRESHOLDS - 1];
        slots.push(EMPTY);
        let current = thresholds_on_one_measure(&slots);
        let empty = threshold_with(&current, EMPTY);
        let mut active = ActiveThresholds::new(&current);
        let log = discard();
        for slot in ModelSlot::ALL {
            assert!(
                active.admits(&log, &update(&empty, slot)),
                "{slot} of the same threshold"
            );
        }
        assert!(!active.admits(&log, &create(MODEL)));
    }

    // At the cap, emptying one of two filled slots keeps a threshold counted, and moving a
    // threshold's only model to the other slot keeps its count.
    #[test]
    fn the_cap_follows_the_slots_a_threshold_keeps() {
        let mut slots = vec![MODEL; MAX_ACTIVE_THRESHOLDS - 1];
        slots[0] = BOTH;
        slots.push(MODEL);
        let current = thresholds_on_one_measure(&slots);
        let both = threshold_with(&current, BOTH);
        let only = current
            .values()
            .find(|threshold| threshold.id != both.id)
            .cloned()
            .expect("a threshold with only a model");
        let mut active = ActiveThresholds::new(&current);
        let log = discard();

        active.release_removed(&ThresholdAction::Remove(both, ModelSlot::Model));
        assert!(!active.admits(&log, &create(MODEL)), "both still counts");

        active.release_removed(&ThresholdAction::Remove(only.clone(), ModelSlot::Model));
        assert!(active.admits(&log, &update(&only, ModelSlot::ChangepointModel)));
        assert!(
            !active.admits(&log, &create(MODEL)),
            "the move kept the count"
        );
    }

    const EMPTY: (bool, bool) = (false, false);
    const MODEL: (bool, bool) = (true, false);
    const CHANGEPOINT: (bool, bool) = (false, true);
    const BOTH: (bool, bool) = (true, true);

    /// One threshold on one branch, testbed, and measure for each pair of filled slots.
    fn thresholds_on_one_measure(
        slots: &[(bool, bool)],
    ) -> HashMap<ThresholdDimensions, QueryThreshold> {
        (1..)
            .zip(slots)
            .map(|(id, &(model, changepoint_model))| {
                let threshold = QueryThreshold {
                    id: ThresholdId::from_raw(id),
                    uuid: bencher_json::ThresholdUuid::new(),
                    project_id: ProjectId::from_raw(1),
                    branch_id: BranchId::from_raw(1),
                    testbed_id: TestbedId::from_raw(1),
                    parameters: None,
                    measure_id: MeasureId::from_raw(1),
                    metric: Some(metric_name(&format!("metric-{id}"))),
                    model_id: model.then(|| ModelId::from_raw(id)),
                    changepoint_model_id: changepoint_model.then(|| ModelId::from_raw(id + 1000)),
                    created: bencher_json::DateTime::TEST,
                    modified: bencher_json::DateTime::TEST,
                };
                (threshold.dimensions(), threshold)
            })
            .collect()
    }

    fn threshold_with(
        thresholds: &HashMap<ThresholdDimensions, QueryThreshold>,
        (model, changepoint_model): (bool, bool),
    ) -> QueryThreshold {
        thresholds
            .values()
            .find(|threshold| {
                threshold.model_id.is_some() == model
                    && threshold.changepoint_model_id.is_some() == changepoint_model
            })
            .cloned()
            .expect("a threshold with these slots")
    }

    /// A create of a new threshold on the same branch, testbed, and measure.
    fn create((model, changepoint_model): (bool, bool)) -> ThresholdAction {
        let dimensions = ThresholdDimensions {
            branch_id: BranchId::from_raw(1),
            testbed_id: TestbedId::from_raw(1),
            parameters: None,
            measure_id: MeasureId::from_raw(1),
            metric: Some(metric_name("created")),
        };
        ThresholdAction::Create(
            dimensions,
            ThresholdSlots {
                model: model.then(Model::upper_boundary),
                changepoint_model: changepoint_model.then(changepoint),
            },
        )
    }

    fn update(threshold: &QueryThreshold, slot: ModelSlot) -> ThresholdAction {
        let model = match slot {
            ModelSlot::Model => Model::upper_boundary(),
            ModelSlot::ChangepointModel => changepoint(),
        };
        ThresholdAction::Update(threshold.clone(), slot, model)
    }

    fn changepoint() -> Model {
        Model {
            test: bencher_json::ModelTest::Changepoint,
            min_sample_size: None,
            max_sample_size: Some(bencher_json::SampleSize::SIXTY_FOUR),
            window: None,
            lower_boundary: None,
            upper_boundary: Some(bencher_json::Boundary::FIFTY),
            significance: Some("0.001".parse().expect("a significance")),
            confirmations: None,
        }
    }
}
