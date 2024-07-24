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
    context::DbConnection,
    error::{assert_parentage, resource_conflict_err, BencherResource},
    model::project::QueryProject,
    schema,
    schema::measure as measure_table,
    util::{
        fn_get::{fn_get, fn_get_id, fn_get_uuid},
        name_id::{fn_eq_name_id, fn_from_name_id},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
    },
};

use super::ProjectId;

crate::util::typed_id::typed_id!(MeasureId);

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
}

impl QueryMeasure {
    fn_eq_resource_id!(measure);
    fn_from_resource_id!(measure, Measure);

    fn_eq_name_id!(ResourceName, measure);
    fn_from_name_id!(measure, Measure);

    fn_get!(measure, MeasureId);
    fn_get_id!(measure, MeasureId, MeasureUuid);
    fn_get_uuid!(measure, MeasureId, MeasureUuid);

    pub fn get_or_create(
        conn: &mut DbConnection,
        project_id: ProjectId,
        measure: &MeasureNameId,
    ) -> Result<MeasureId, HttpError> {
        let query_measure = Self::from_name_id(conn, project_id, measure);

        let http_error = match query_measure {
            Ok(measure) => return Ok(measure.id),
            Err(e) => e,
        };

        // Dynamically create adapter specific measures
        // Or recreate default measures if they were previously deleted
        let measure_str = measure.as_ref();

        let measure = if let Some(measure) = built_in::generic::Latency::from_name_id(measure_str)
            .or_else(|| built_in::generic::Throughput::from_name_id(measure_str))
            .or_else(|| built_in::iai::Instructions::from_name_id(measure_str))
            .or_else(|| built_in::iai::L1Accesses::from_name_id(measure_str))
            .or_else(|| built_in::iai::L2Accesses::from_name_id(measure_str))
            .or_else(|| built_in::iai::RamAccesses::from_name_id(measure_str))
            .or_else(|| built_in::iai::EstimatedCycles::from_name_id(measure_str))
            .or_else(|| {
                built_in::iai_callgrind::callgrind_tool::Instructions::from_name_id(measure_str)
            })
            .or_else(|| built_in::iai_callgrind::callgrind_tool::L1Hits::from_name_id(measure_str))
            .or_else(|| built_in::iai_callgrind::callgrind_tool::L2Hits::from_name_id(measure_str))
            .or_else(|| built_in::iai_callgrind::callgrind_tool::RamHits::from_name_id(measure_str))
            .or_else(|| {
                built_in::iai_callgrind::callgrind_tool::TotalReadWrite::from_name_id(measure_str)
            })
            .or_else(|| {
                built_in::iai_callgrind::callgrind_tool::EstimatedCycles::from_name_id(measure_str)
            })
            .or_else(|| built_in::iai_callgrind::dhat_tool::TotalBytes::from_name_id(measure_str))
            .or_else(|| built_in::iai_callgrind::dhat_tool::TotalBlocks::from_name_id(measure_str))
            .or_else(|| built_in::iai_callgrind::dhat_tool::AtTGmaxBytes::from_name_id(measure_str))
            .or_else(|| {
                built_in::iai_callgrind::dhat_tool::AtTGmaxBlocks::from_name_id(measure_str)
            })
            .or_else(|| built_in::iai_callgrind::dhat_tool::AtTEndBytes::from_name_id(measure_str))
            .or_else(|| built_in::iai_callgrind::dhat_tool::AtTEndBlocks::from_name_id(measure_str))
            .or_else(|| built_in::iai_callgrind::dhat_tool::ReadsBytes::from_name_id(measure_str))
            .or_else(|| built_in::iai_callgrind::dhat_tool::WritesBytes::from_name_id(measure_str))
            .or_else(|| built_in::file_size::FileSize::from_name_id(measure_str))
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

        let insert_measure = InsertMeasure::from_json(conn, project_id, measure)?;
        diesel::insert_into(schema::measure::table)
            .values(&insert_measure)
            .execute(conn)
            .map_err(resource_conflict_err!(Measure, insert_measure))?;

        Self::get_id(conn, insert_measure.uuid)
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
}

impl InsertMeasure {
    pub fn from_measure<T: BuiltInMeasure>(
        conn: &mut DbConnection,
        project_id: ProjectId,
    ) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, T::new_json())
    }

    pub fn from_json(
        conn: &mut DbConnection,
        project_id: ProjectId,
        measure: JsonNewMeasure,
    ) -> Result<Self, HttpError> {
        let JsonNewMeasure { name, slug, units } = measure;
        let slug = ok_slug!(conn, project_id, &name, slug, measure, QueryMeasure)?;
        let timestamp = DateTime::now();
        Ok(Self {
            uuid: MeasureUuid::new(),
            project_id,
            name,
            slug,
            units,
            created: timestamp,
            modified: timestamp,
        })
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = measure_table)]
pub struct UpdateMeasure {
    pub name: Option<ResourceName>,
    pub slug: Option<Slug>,
    pub units: Option<ResourceName>,
    pub modified: DateTime,
}

impl From<JsonUpdateMeasure> for UpdateMeasure {
    fn from(update: JsonUpdateMeasure) -> Self {
        let JsonUpdateMeasure { name, slug, units } = update;
        Self {
            name,
            slug,
            units,
            modified: DateTime::now(),
        }
    }
}
