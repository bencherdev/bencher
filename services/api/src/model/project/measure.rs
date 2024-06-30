use bencher_json::{
    project::measure::{
        JsonUpdateMeasure, MeasureUuid, ESTIMATED_CYCLES_NAME_STR, ESTIMATED_CYCLES_SLUG_STR,
        FILE_SIZE_NAME_STR, FILE_SIZE_SLUG_STR, INSTRUCTIONS_NAME_STR, INSTRUCTIONS_SLUG_STR,
        L1_ACCESSES_NAME_STR, L1_ACCESSES_SLUG_STR, L2_ACCESSES_NAME_STR, L2_ACCESSES_SLUG_STR,
        LATENCY_NAME_STR, LATENCY_SLUG_STR, MEASURE_UNITS, RAM_ACCESSES_NAME_STR,
        RAM_ACCESSES_SLUG_STR, THROUGHPUT_NAME_STR, THROUGHPUT_SLUG_STR, TOTAL_ACCESSES_NAME_STR,
        TOTAL_ACCESSES_SLUG_STR,
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
        let measure = match measure.as_ref() {
            // Recreate
            LATENCY_NAME_STR | LATENCY_SLUG_STR => JsonNewMeasure::latency(),
            THROUGHPUT_NAME_STR | THROUGHPUT_SLUG_STR => JsonNewMeasure::throughput(),
            // Adapter specific
            // Iai
            INSTRUCTIONS_NAME_STR | INSTRUCTIONS_SLUG_STR => JsonNewMeasure::instructions(),
            L1_ACCESSES_NAME_STR | L1_ACCESSES_SLUG_STR => JsonNewMeasure::l1_accesses(),
            L2_ACCESSES_NAME_STR | L2_ACCESSES_SLUG_STR => JsonNewMeasure::l2_accesses(),
            RAM_ACCESSES_NAME_STR | RAM_ACCESSES_SLUG_STR => JsonNewMeasure::ram_accesses(),
            TOTAL_ACCESSES_NAME_STR | TOTAL_ACCESSES_SLUG_STR => JsonNewMeasure::total_accesses(),
            ESTIMATED_CYCLES_NAME_STR | ESTIMATED_CYCLES_SLUG_STR => {
                JsonNewMeasure::estimated_cycles()
            },
            // File size
            FILE_SIZE_NAME_STR | FILE_SIZE_SLUG_STR => JsonNewMeasure::file_size(),
            _ => {
                let Ok(kind) = NameIdKind::<ResourceName>::try_from(measure) else {
                    return Err(http_error);
                };
                match kind {
                    NameIdKind::Uuid(_) => return Err(http_error),
                    NameIdKind::Slug(slug) => JsonNewMeasure {
                        name: slug.clone().into(),
                        slug: Some(slug),
                        units: MEASURE_UNITS.clone(),
                    },
                    NameIdKind::Name(name) => JsonNewMeasure {
                        name,
                        slug: None,
                        units: MEASURE_UNITS.clone(),
                    },
                }
            },
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

    pub fn latency(conn: &mut DbConnection, project_id: ProjectId) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewMeasure::latency())
    }

    pub fn throughput(conn: &mut DbConnection, project_id: ProjectId) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewMeasure::throughput())
    }

    pub fn instructions(conn: &mut DbConnection, project_id: ProjectId) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewMeasure::instructions())
    }

    pub fn l1_accesses(conn: &mut DbConnection, project_id: ProjectId) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewMeasure::l1_accesses())
    }

    pub fn l2_accesses(conn: &mut DbConnection, project_id: ProjectId) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewMeasure::l2_accesses())
    }

    pub fn ram_accesses(conn: &mut DbConnection, project_id: ProjectId) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewMeasure::ram_accesses())
    }

    pub fn total_accesses(
        conn: &mut DbConnection,
        project_id: ProjectId,
    ) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewMeasure::total_accesses())
    }

    pub fn estimated_cycles(
        conn: &mut DbConnection,
        project_id: ProjectId,
    ) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewMeasure::estimated_cycles())
    }

    pub fn file_size(conn: &mut DbConnection, project_id: ProjectId) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewMeasure::file_size())
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
