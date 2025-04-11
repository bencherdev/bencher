use bencher_json::{
    project::measure::{
        built_in::{self, BuiltInMeasure},
        JsonUpdateMeasure, MeasureUuid,
    },
    DateTime, JsonMeasure, JsonNewMeasure, MeasureNameId, NameIdKind, ResourceName, Slug,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{assert_parentage, resource_conflict_err, BencherResource},
    macros::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        name_id::{fn_eq_name_id, fn_from_name_id},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
    },
    model::project::QueryProject,
    schema::{self, measure as measure_table},
};

use super::ProjectId;

crate::macros::typed_id::typed_id!(MeasureId);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = measure_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryMeasure {
    pub id: MeasureId,
    pub uuid: MeasureUuid,
    pub project_id: ProjectId,
    pub name: ResourceName,
    pub slug: Slug,
    pub units: ResourceName,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl QueryMeasure {
    fn_eq_resource_id!(measure);
    fn_from_resource_id!(project_id, ProjectId, measure, Measure);

    fn_eq_name_id!(ResourceName, measure);
    fn_from_name_id!(measure, Measure);

    fn_get!(measure, MeasureId);
    fn_get_id!(measure, MeasureId, MeasureUuid);
    fn_get_uuid!(measure, MeasureId, MeasureUuid);
    fn_from_uuid!(measure, MeasureUuid, Measure);

    pub async fn get_or_create(
        context: &ApiContext,
        project_id: ProjectId,
        measure: &MeasureNameId,
    ) -> Result<MeasureId, HttpError> {
        let query_measure = Self::get_or_create_inner(context, project_id, measure).await?;

        if query_measure.archived.is_some() {
            let update_measure = UpdateMeasure::unarchive();
            diesel::update(schema::measure::table.filter(schema::measure::id.eq(query_measure.id)))
                .set(&update_measure)
                .execute(conn_lock!(context))
                .map_err(resource_conflict_err!(Benchmark, &query_measure))?;
        }

        Ok(query_measure.id)
    }

    async fn get_or_create_inner(
        context: &ApiContext,
        project_id: ProjectId,
        measure: &MeasureNameId,
    ) -> Result<Self, HttpError> {
        let query_measure = Self::from_name_id(conn_lock!(context), project_id, measure);

        let http_error = match query_measure {
            Ok(measure) => return Ok(measure),
            Err(e) => e,
        };

        // Dynamically create adapter specific measures
        // Or recreate default measures if they were previously deleted
        let measure_str = measure.as_ref();

        let json_measure = if let Some(measure) = built_in::default::Latency::from_str(measure_str)
            .or_else(|| built_in::default::Throughput::from_str(measure_str))
            .or_else(|| built_in::json::BuildTime::from_str(measure_str))
            .or_else(|| built_in::json::FileSize::from_str(measure_str))
            .or_else(|| built_in::iai::Instructions::from_str(measure_str))
            .or_else(|| built_in::iai::L1Accesses::from_str(measure_str))
            .or_else(|| built_in::iai::L2Accesses::from_str(measure_str))
            .or_else(|| built_in::iai::RamAccesses::from_str(measure_str))
            .or_else(|| built_in::iai::EstimatedCycles::from_str(measure_str))
            .or_else(|| built_in::iai_callgrind::Instructions::from_str(measure_str))
            .or_else(|| built_in::iai_callgrind::L1Hits::from_str(measure_str))
            .or_else(|| built_in::iai_callgrind::L2Hits::from_str(measure_str))
            .or_else(|| built_in::iai_callgrind::RamHits::from_str(measure_str))
            .or_else(|| built_in::iai_callgrind::TotalReadWrite::from_str(measure_str))
            .or_else(|| built_in::iai_callgrind::EstimatedCycles::from_str(measure_str))
            .or_else(|| built_in::iai_callgrind::GlobalBusEvents::from_str(measure_str))
            .or_else(|| built_in::iai_callgrind::TotalBytes::from_str(measure_str))
            .or_else(|| built_in::iai_callgrind::TotalBlocks::from_str(measure_str))
            .or_else(|| built_in::iai_callgrind::AtTGmaxBytes::from_str(measure_str))
            .or_else(|| built_in::iai_callgrind::AtTGmaxBlocks::from_str(measure_str))
            .or_else(|| built_in::iai_callgrind::AtTEndBytes::from_str(measure_str))
            .or_else(|| built_in::iai_callgrind::AtTEndBlocks::from_str(measure_str))
            .or_else(|| built_in::iai_callgrind::ReadsBytes::from_str(measure_str))
            .or_else(|| built_in::iai_callgrind::WritesBytes::from_str(measure_str))
        {
            measure
        } else {
            let Ok(kind) = NameIdKind::<ResourceName>::try_from(measure) else {
                return Err(http_error);
            };
            match kind {
                NameIdKind::Uuid(_) => return Err(http_error),
                NameIdKind::Slug(slug) => JsonNewMeasure {
                    name: slug.clone().into(),
                    slug: Some(slug),
                    units: JsonNewMeasure::generic_unit(),
                },
                NameIdKind::Name(name) => JsonNewMeasure {
                    name,
                    slug: None,
                    units: JsonNewMeasure::generic_unit(),
                },
            }
        };

        Self::create(context, project_id, json_measure).await
    }

    pub async fn create(
        context: &ApiContext,
        project_id: ProjectId,
        json_measure: JsonNewMeasure,
    ) -> Result<Self, HttpError> {
        InsertMeasure::rate_limit(context, project_id).await?;

        let insert_measure =
            InsertMeasure::from_json(conn_lock!(context), project_id, json_measure);
        diesel::insert_into(schema::measure::table)
            .values(&insert_measure)
            .execute(conn_lock!(context))
            .map_err(resource_conflict_err!(Measure, insert_measure))?;

        Self::from_uuid(conn_lock!(context), project_id, insert_measure.uuid)
    }

    pub fn into_json_for_project(self, project: &QueryProject) -> JsonMeasure {
        let Self {
            uuid,
            project_id,
            name,
            slug,
            units,
            created,
            modified,
            archived,
            ..
        } = self;
        assert_parentage(
            BencherResource::Project,
            project.id,
            BencherResource::Measure,
            project_id,
        );
        JsonMeasure {
            uuid,
            project: project.uuid,
            name,
            slug,
            units,
            created,
            modified,
            archived,
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = measure_table)]
pub struct InsertMeasure {
    pub uuid: MeasureUuid,
    pub project_id: ProjectId,
    pub name: ResourceName,
    pub slug: Slug,
    pub units: ResourceName,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl InsertMeasure {
    #[cfg(feature = "plus")]
    crate::model::rate_limit::fn_rate_limit!(measure, Measure);

    pub fn from_measure<T: BuiltInMeasure>(conn: &mut DbConnection, project_id: ProjectId) -> Self {
        Self::from_json(conn, project_id, T::new_json())
    }

    fn from_json(conn: &mut DbConnection, project_id: ProjectId, measure: JsonNewMeasure) -> Self {
        let JsonNewMeasure { name, slug, units } = measure;
        let slug = ok_slug!(conn, project_id, &name, slug, measure, QueryMeasure);
        let timestamp = DateTime::now();
        Self {
            uuid: MeasureUuid::new(),
            project_id,
            name,
            slug,
            units,
            created: timestamp,
            modified: timestamp,
            archived: None,
        }
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = measure_table)]
pub struct UpdateMeasure {
    pub name: Option<ResourceName>,
    pub slug: Option<Slug>,
    pub units: Option<ResourceName>,
    pub modified: DateTime,
    pub archived: Option<Option<DateTime>>,
}

impl From<JsonUpdateMeasure> for UpdateMeasure {
    fn from(update: JsonUpdateMeasure) -> Self {
        let JsonUpdateMeasure {
            name,
            slug,
            units,
            archived,
        } = update;
        let modified = DateTime::now();
        let archived = archived.map(|archived| archived.then_some(modified));
        Self {
            name,
            slug,
            units,
            modified,
            archived,
        }
    }
}

impl UpdateMeasure {
    fn unarchive() -> Self {
        JsonUpdateMeasure {
            name: None,
            slug: None,
            units: None,
            archived: Some(false),
        }
        .into()
    }
}
