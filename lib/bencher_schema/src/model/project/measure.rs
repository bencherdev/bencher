use bencher_json::{
    DateTime, JsonMeasure, JsonNewMeasure, MeasureNameId, MeasureSlug, NameId, ResourceName,
    project::measure::{
        JsonUpdateMeasure, MeasureUuid,
        built_in::{self, BuiltInMeasure},
    },
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::{BencherResource, assert_parentage, resource_conflict_err},
    macros::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        name_id::{fn_eq_name_id, fn_from_name_id},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
    },
    model::project::QueryProject,
    schema::{self, measure as measure_table},
    write_conn,
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
    pub slug: MeasureSlug,
    pub units: ResourceName,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl QueryMeasure {
    fn_eq_resource_id!(measure, MeasureResourceId);
    fn_from_resource_id!(project_id, ProjectId, measure, Measure, MeasureResourceId);

    fn_eq_name_id!(ResourceName, measure, MeasureNameId);
    fn_from_name_id!(measure, Measure, MeasureNameId);

    fn_get!(measure, MeasureId);
    fn_get_id!(measure, MeasureId, MeasureUuid);
    fn_get_uuid!(measure, MeasureId, MeasureUuid);
    fn_from_uuid!(project_id, ProjectId, measure, MeasureUuid, Measure);

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
                .execute(write_conn!(context))
                .map_err(resource_conflict_err!(Measure, &query_measure))?;
        }

        Ok(query_measure.id)
    }

    async fn get_or_create_inner(
        context: &ApiContext,
        project_id: ProjectId,
        measure: &MeasureNameId,
    ) -> Result<Self, HttpError> {
        let query_measure = Self::from_name_id(auth_conn!(context), project_id, measure);

        let http_error = match query_measure {
            Ok(measure) => return Ok(measure),
            Err(e) => e,
        };

        // Dynamically create adapter specific measures
        // Or recreate default measures if they were previously deleted
        let measure_str = &measure.to_string();

        let json_measure = if let Some(measure) = built_in::default::Latency::from_str(measure_str)
            .or_else(|| built_in::default::Throughput::from_str(measure_str))
            .or_else(|| built_in::json::BuildTime::from_str(measure_str))
            .or_else(|| built_in::json::FileSize::from_str(measure_str))
            .or_else(|| built_in::iai::Instructions::from_str(measure_str))
            .or_else(|| built_in::iai::L1Accesses::from_str(measure_str))
            .or_else(|| built_in::iai::L2Accesses::from_str(measure_str))
            .or_else(|| built_in::iai::RamAccesses::from_str(measure_str))
            .or_else(|| built_in::iai::EstimatedCycles::from_str(measure_str))
            // Gungraun:
            // callgrind/cachegrind
            .or_else(|| built_in::gungraun::Instructions::from_str(measure_str))
            .or_else(|| built_in::gungraun::L1Hits::from_str(measure_str))
            .or_else(|| built_in::gungraun::L2Hits::from_str(measure_str))
            .or_else(|| built_in::gungraun::LLHits::from_str(measure_str))
            .or_else(|| built_in::gungraun::RamHits::from_str(measure_str))
            .or_else(|| built_in::gungraun::TotalReadWrite::from_str(measure_str))
            .or_else(|| built_in::gungraun::EstimatedCycles::from_str(measure_str))
            .or_else(|| built_in::gungraun::GlobalBusEvents::from_str(measure_str))
            .or_else(|| built_in::gungraun::Dr::from_str(measure_str))
            .or_else(|| built_in::gungraun::Dw::from_str(measure_str))
            .or_else(|| built_in::gungraun::I1mr::from_str(measure_str))
            .or_else(|| built_in::gungraun::D1mr::from_str(measure_str))
            .or_else(|| built_in::gungraun::D1mw::from_str(measure_str))
            .or_else(|| built_in::gungraun::ILmr::from_str(measure_str))
            .or_else(|| built_in::gungraun::DLmr::from_str(measure_str))
            .or_else(|| built_in::gungraun::DLmw::from_str(measure_str))
            .or_else(|| built_in::gungraun::I1MissRate::from_str(measure_str))
            .or_else(|| built_in::gungraun::LLiMissRate::from_str(measure_str))
            .or_else(|| built_in::gungraun::D1MissRate::from_str(measure_str))
            .or_else(|| built_in::gungraun::LLdMissRate::from_str(measure_str))
            .or_else(|| built_in::gungraun::LLMissRate::from_str(measure_str))
            .or_else(|| built_in::gungraun::L1HitRate::from_str(measure_str))
            .or_else(|| built_in::gungraun::LLHitRate::from_str(measure_str))
            .or_else(|| built_in::gungraun::RamHitRate::from_str(measure_str))
            .or_else(|| built_in::gungraun::SysCount::from_str(measure_str))
            .or_else(|| built_in::gungraun::SysTime::from_str(measure_str))
            .or_else(|| built_in::gungraun::SysCpuTime::from_str(measure_str))
            .or_else(|| built_in::gungraun::Bc::from_str(measure_str))
            .or_else(|| built_in::gungraun::Bcm::from_str(measure_str))
            .or_else(|| built_in::gungraun::Bi::from_str(measure_str))
            .or_else(|| built_in::gungraun::Bim::from_str(measure_str))
            .or_else(|| built_in::gungraun::ILdmr::from_str(measure_str))
            .or_else(|| built_in::gungraun::DLdmr::from_str(measure_str))
            .or_else(|| built_in::gungraun::DLdmw::from_str(measure_str))
            .or_else(|| built_in::gungraun::AcCost1::from_str(measure_str))
            .or_else(|| built_in::gungraun::AcCost2::from_str(measure_str))
            .or_else(|| built_in::gungraun::SpLoss1::from_str(measure_str))
            .or_else(|| built_in::gungraun::SpLoss2::from_str(measure_str))
            // dhat
            .or_else(|| built_in::gungraun::TotalBytes::from_str(measure_str))
            .or_else(|| built_in::gungraun::TotalBlocks::from_str(measure_str))
            .or_else(|| built_in::gungraun::AtTGmaxBytes::from_str(measure_str))
            .or_else(|| built_in::gungraun::AtTGmaxBlocks::from_str(measure_str))
            .or_else(|| built_in::gungraun::AtTEndBytes::from_str(measure_str))
            .or_else(|| built_in::gungraun::AtTEndBlocks::from_str(measure_str))
            .or_else(|| built_in::gungraun::ReadsBytes::from_str(measure_str))
            .or_else(|| built_in::gungraun::WritesBytes::from_str(measure_str))
            // memcheck
            .or_else(|| built_in::gungraun::MemcheckErrors::from_str(measure_str))
            .or_else(|| built_in::gungraun::MemcheckContexts::from_str(measure_str))
            .or_else(|| built_in::gungraun::MemcheckSuppressedErrors::from_str(measure_str))
            .or_else(|| built_in::gungraun::MemcheckSuppressedContexts::from_str(measure_str))
            // helgrind
            .or_else(|| built_in::gungraun::HelgrindErrors::from_str(measure_str))
            .or_else(|| built_in::gungraun::HelgrindContexts::from_str(measure_str))
            .or_else(|| built_in::gungraun::HelgrindSuppressedErrors::from_str(measure_str))
            .or_else(|| built_in::gungraun::HelgrindSuppressedContexts::from_str(measure_str))
            // drd
            .or_else(|| built_in::gungraun::DrdErrors::from_str(measure_str))
            .or_else(|| built_in::gungraun::DrdContexts::from_str(measure_str))
            .or_else(|| built_in::gungraun::DrdSuppressedErrors::from_str(measure_str))
            .or_else(|| built_in::gungraun::DrdSuppressedContexts::from_str(measure_str))
        {
            measure
        } else {
            match measure.clone() {
                NameId::Uuid(_) => return Err(http_error),
                NameId::Slug(slug) => JsonNewMeasure {
                    name: slug.clone().into(),
                    slug: Some(slug),
                    units: JsonNewMeasure::generic_unit(),
                },
                NameId::Name(name) => JsonNewMeasure {
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
        #[cfg(feature = "plus")]
        InsertMeasure::rate_limit(context, project_id).await?;

        let insert_measure =
            InsertMeasure::from_json(auth_conn!(context), project_id, json_measure);
        diesel::insert_into(schema::measure::table)
            .values(&insert_measure)
            .execute(write_conn!(context))
            .map_err(resource_conflict_err!(Measure, insert_measure))?;

        Self::from_uuid(auth_conn!(context), project_id, insert_measure.uuid)
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
    pub slug: MeasureSlug,
    pub units: ResourceName,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl InsertMeasure {
    #[cfg(feature = "plus")]
    crate::macros::rate_limit::fn_rate_limit!(measure, Measure);

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
    pub slug: Option<MeasureSlug>,
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
