use bencher_json::{
    project::measure::{
        defs::{self, MeasureDefinition},
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
        fn test<T: MeasureDefinition>(measure_str: &str) -> Option<JsonNewMeasure> {
            (measure_str == T::NAME_STR || measure_str == T::SLUG_STR).then(T::json_new)
        }

        let query_measure = Self::from_name_id(conn, project_id, measure);

        let http_error = match query_measure {
            Ok(measure) => return Ok(measure.id),
            Err(e) => e,
        };

        // Dynamically create adapter specific measures
        // Or recreate default measures if they were previously deleted
        let measure_str = measure.as_ref();

        let measure = if let Some(measure) = test::<defs::generic::Latency>(measure_str)
            .or_else(|| test::<defs::generic::Throughput>(measure_str))
            .or_else(|| test::<defs::iai::Instructions>(measure_str))
            .or_else(|| test::<defs::iai::L1Accesses>(measure_str))
            .or_else(|| test::<defs::iai::L2Accesses>(measure_str))
            .or_else(|| test::<defs::iai::RamAccesses>(measure_str))
            .or_else(|| test::<defs::iai::EstimatedCycles>(measure_str))
            .or_else(|| test::<defs::iai_callgrind::callgrind_tool::Instructions>(measure_str))
            .or_else(|| test::<defs::iai_callgrind::callgrind_tool::L1Hits>(measure_str))
            .or_else(|| test::<defs::iai_callgrind::callgrind_tool::L2Hits>(measure_str))
            .or_else(|| test::<defs::iai_callgrind::callgrind_tool::RamHits>(measure_str))
            .or_else(|| test::<defs::iai_callgrind::callgrind_tool::TotalReadWrite>(measure_str))
            .or_else(|| test::<defs::iai_callgrind::callgrind_tool::EstimatedCycles>(measure_str))
            .or_else(|| test::<defs::iai_callgrind::dhat_tool::TotalBytes>(measure_str))
            .or_else(|| test::<defs::iai_callgrind::dhat_tool::TotalBlocks>(measure_str))
            .or_else(|| test::<defs::iai_callgrind::dhat_tool::AtTGmaxBytes>(measure_str))
            .or_else(|| test::<defs::iai_callgrind::dhat_tool::AtTGmaxBlocks>(measure_str))
            .or_else(|| test::<defs::iai_callgrind::dhat_tool::AtTEndBytes>(measure_str))
            .or_else(|| test::<defs::iai_callgrind::dhat_tool::AtTEndBlocks>(measure_str))
            .or_else(|| test::<defs::iai_callgrind::dhat_tool::ReadsBytes>(measure_str))
            .or_else(|| test::<defs::iai_callgrind::dhat_tool::WritesBytes>(measure_str))
            .or_else(|| test::<defs::file_size::FileSize>(measure_str))
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
    pub fn from_measure<T: MeasureDefinition>(
        conn: &mut DbConnection,
        project_id: ProjectId,
    ) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, T::json_new())
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
