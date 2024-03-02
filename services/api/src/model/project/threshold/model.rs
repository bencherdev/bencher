use bencher_json::{
    Boundary, DateTime, JsonModel, Model, ModelTest, ModelUuid, SampleSize, Window,
};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::{assert_parentage, BencherResource},
    schema::model as model_table,
    util::fn_get::{fn_get, fn_get_id, fn_get_uuid},
};

use super::{QueryThreshold, ThresholdId};

crate::util::typed_id::typed_id!(ModelId);

#[derive(Debug, Clone, diesel::Queryable, diesel::Selectable)]
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

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonModel, HttpError> {
        let threshold = QueryThreshold::get(conn, self.threshold_id)?;
        Ok(self.into_json_for_threshold(&threshold))
    }

    pub fn into_json_for_threshold(self, threshold: &QueryThreshold) -> JsonModel {
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
            threshold.id,
            BencherResource::Model,
            threshold_id,
        );
        JsonModel {
            uuid,
            threshold: threshold.uuid,
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

impl From<QueryModel> for InsertModel {
    fn from(query_model: QueryModel) -> Self {
        let QueryModel {
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

impl InsertModel {
    pub fn from_json(threshold_id: ThresholdId, model: Model) -> Self {
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
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = model_table)]
pub struct UpdateModel {
    pub replaced: DateTime,
}

impl UpdateModel {
    pub fn replace() -> Result<Self, HttpError> {
        Ok(Self {
            replaced: DateTime::now(),
        })
    }
}
