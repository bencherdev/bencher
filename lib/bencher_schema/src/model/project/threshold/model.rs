use bencher_json::{
    Boundary, DateTime, JsonModel, Model, ModelTest, ModelUuid, SampleSize, Window,
};
use diesel::{ExpressionMethods as _, JoinOnDsl as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _};
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
    #[cfg(feature = "plus")]
    pub async fn rate_limit(
        context: &crate::ApiContext,
        query_threshold: &QueryThreshold,
    ) -> Result<(), HttpError> {
        use crate::{conn_lock, context::RateLimitingError, error::issue_error};

        let resource = BencherResource::Model;
        let (start_time, end_time) = context.rate_limiting.window();
        let creation_count: u32 = schema::model::table
                .filter(schema::model::threshold_id.eq(query_threshold.id))
                .filter(schema::model::created.ge(start_time))
                .filter(schema::model::created.le(end_time))
                .count()
                .get_result::<i64>(conn_lock!(context))
                .map_err(resource_not_found_err!(Model, (query_threshold, start_time, end_time)))?
                .try_into()
                .map_err(|e| {
                    issue_error(
                        "Failed to count creation",
                        &format!("Failed to count {resource} creation for threshold ({uuid}) between {start_time} and {end_time}.", uuid = query_threshold.uuid),
                    e
                    )}
                )?;

        // The only way that new Model can be crated is either through running a Report
        // or by updating an existing threshold using the API.
        // The running of a Report will be rate limited already for unclaimed projects,
        // and the API endpoint to update an existing threshold would require authentication and would therefore be a claimed project.
        let rate_limit = context.rate_limiting.claimed_limit;
        if creation_count >= rate_limit {
            Err(crate::error::too_many_requests(
                RateLimitingError::Threshold {
                    threshold: query_threshold.clone(),
                    resource,
                    rate_limit,
                },
            ))
        } else {
            Ok(())
        }
    }

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
