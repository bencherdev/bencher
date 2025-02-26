use bencher_json::{
    Boundary, DateTime, JsonModel, Model, ModelTest, ModelUuid, SampleSize, Window,
};
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl, SelectableHelper};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::{assert_parentage, resource_not_found_err, BencherResource},
    macros::fn_get::{fn_get, fn_get_id, fn_get_uuid},
    model::project::ProjectId,
    schema::{self, model as model_table},
};

use super::{QueryThreshold, ThresholdId};

crate::macros::typed_id::typed_id!(ModelId);

#[derive(Debug, Clone, Copy, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = model_table)]
pub struct QueryModel {
    pub id: ModelId,
    pub uuid: ModelUuid,
    pub threshold_id: ThresholdId,
    pub test: ModelTest,
    pub min_sample_size: Option<SampleSize>,
    pub max_sample_size: Option<SampleSize>,
    pub window: Option<Window>,
    pub lower_boundary: Option<Boundary>,
    pub upper_boundary: Option<Boundary>,
    pub created: DateTime,
    pub replaced: Option<DateTime>,
}

impl QueryModel {
    fn_get!(model, ModelId);
    fn_get_id!(model, ModelId, ModelUuid);
    fn_get_uuid!(model, ModelId, ModelUuid);

    pub fn from_uuid(
        conn: &mut DbConnection,
        project_id: ProjectId,
        model_uuid: ModelUuid,
    ) -> Result<Self, HttpError> {
        schema::model::table
            .inner_join(
                schema::threshold::table.on(schema::model::threshold_id.eq(schema::threshold::id)),
            )
            .filter(schema::threshold::project_id.eq(project_id))
            .filter(schema::model::uuid.eq(model_uuid))
            .select(Self::as_select())
            .first(conn)
            .map_err(resource_not_found_err!(Model, (project_id, model_uuid)))
    }

    pub fn into_model(self) -> Model {
        let Self {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
            ..
        } = self;
        Model {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        }
    }

    pub fn into_json(self, query_threshold: &QueryThreshold) -> JsonModel {
        let Self {
            uuid,
            threshold_id,
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
            created,
            replaced,
            ..
        } = self;
        assert_parentage(
            BencherResource::Threshold,
            query_threshold.id,
            BencherResource::Model,
            threshold_id,
        );
        JsonModel {
            uuid,
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
            created,
            replaced,
        }
    }
}

#[derive(Debug, Clone, diesel::Insertable)]
#[diesel(table_name = model_table)]
pub struct InsertModel {
    pub uuid: ModelUuid,
    pub threshold_id: ThresholdId,
    pub test: ModelTest,
    pub min_sample_size: Option<SampleSize>,
    pub max_sample_size: Option<SampleSize>,
    pub window: Option<Window>,
    pub lower_boundary: Option<Boundary>,
    pub upper_boundary: Option<Boundary>,
    pub created: DateTime,
    pub replaced: Option<DateTime>,
}

impl InsertModel {
    pub fn new(threshold_id: ThresholdId, model: Model) -> Self {
        let Model {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        } = model;
        Self {
            uuid: ModelUuid::new(),
            threshold_id,
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
            created: DateTime::now(),
            replaced: None,
        }
    }

    pub fn with_threshold_id(query_model: QueryModel, threshold_id: ThresholdId) -> Self {
        let QueryModel {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
            created,
            replaced,
            ..
        } = query_model;
        Self {
            uuid: ModelUuid::new(),
            threshold_id,
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
            created,
            replaced,
        }
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = model_table)]
pub struct UpdateModel {
    pub replaced: DateTime,
}

impl UpdateModel {
    pub fn replaced_at(replaced: DateTime) -> Self {
        Self { replaced }
    }
}
