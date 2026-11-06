use bencher_json::{
    DateTime, JsonMeasure, JsonNewMeasure, MeasureNameId, MeasureSlug, NameId, ResourceName, Slug,
    project::measure::{
        JsonUpdateMeasure, MeasureUuid,
        built_in::{self, BuiltInMeasure},
    },
};
use diesel::{ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _};
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
        sql::last_insert_rowid,
    },
    model::project::QueryProject,
    schema::{self, measure as measure_table},
    write_conn, write_transaction,
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
        Self::get_or_create_with_units(context, project_id, measure, None).await
    }

    /// [`Self::get_or_create`], creating a measure that is not built in with `units` when given,
    /// and giving `units` to a measure that still has the generic unit.
    pub async fn get_or_create_with_units(
        context: &ApiContext,
        project_id: ProjectId,
        measure: &MeasureNameId,
        units: Option<&ResourceName>,
    ) -> Result<MeasureId, HttpError> {
        let mut query_measure =
            Self::get_or_create_inner(context, project_id, measure, units).await?;

        if query_measure.archived.is_some() {
            let update_measure = UpdateMeasure::unarchive();
            diesel::update(schema::measure::table.filter(schema::measure::id.eq(query_measure.id)))
                .set(&update_measure)
                .execute(write_conn!(context))
                .map_err(resource_conflict_err!(Measure, &query_measure))?;
            query_measure.archived = None;
        }

        if let Some(units) = units
            && query_measure.units == JsonNewMeasure::generic_unit()
        {
            let update_measure = UpdateMeasure::units(units.clone());
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
        units: Option<&ResourceName>,
    ) -> Result<Self, HttpError> {
        let query_measure = Self::from_name_id(auth_conn!(context), project_id, measure);

        let http_error = match query_measure {
            Ok(measure) => return Ok(measure),
            Err(e) => e,
        };

        let Some(json_measure) = new_measure_with_units(measure, units) else {
            return Err(http_error);
        };

        match Self::create(context, project_id, json_measure).await {
            Ok(measure) => Ok(measure),
            Err(e) if crate::error::is_conflict(&e) => {
                // Another concurrent request created this measure — re-lookup
                Self::from_name_id(auth_conn!(context), project_id, measure)
                    .map_err(|_lookup_err| e)
            },
            Err(e) => Err(e),
        }
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

        write_transaction!(context, |conn| {
            diesel::insert_into(schema::measure::table)
                .values(&insert_measure)
                .execute(conn)?;
            diesel::select(last_insert_rowid()).get_result(conn)
        })
        .map_err(resource_conflict_err!(Measure, &insert_measure))
        .map(|id| insert_measure.into_query(id))
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

    /// What `measure` names in the project, resolved as [`Self::get_or_create`] resolves it
    /// but without creating anything, or `None` for a UUID the project does not hold.
    pub fn identity(
        conn: &mut DbConnection,
        project_id: ProjectId,
        measure: &MeasureNameId,
    ) -> diesel::QueryResult<Option<MeasureIdentity>> {
        let measure_id = schema::measure::table
            .filter(schema::measure::project_id.eq(project_id))
            .filter(Self::eq_name_id(measure))
            .select(schema::measure::id)
            .first::<MeasureId>(conn)
            .optional()?;
        if let Some(measure_id) = measure_id {
            return Ok(Some(MeasureIdentity::Existing(measure_id)));
        }
        let Some(JsonNewMeasure { name, slug, .. }) = new_measure(measure) else {
            return Ok(None);
        };
        Ok(Some(
            match slug.or_else(|| Slug::new(&name).map(Into::into)) {
                Some(slug) => MeasureIdentity::New(slug),
                None => MeasureIdentity::Unslugged(name),
            },
        ))
    }
}

/// What a measure name resolves to in a project: a measure it holds, or the measure it
/// would create, by its slug, or by its name when the name makes no slug.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MeasureIdentity {
    Existing(MeasureId),
    New(MeasureSlug),
    Unslugged(ResourceName),
}

/// The measure a name that the project does not hold creates, or `None` for a UUID.
///
/// A built-in measure is created whole, which also recreates a deleted default measure.
fn new_measure(measure: &MeasureNameId) -> Option<JsonNewMeasure> {
    new_measure_with_units(measure, None)
}

/// [`new_measure`], in `units` when the measure is not built in, else in the generic unit.
fn new_measure_with_units(
    measure: &MeasureNameId,
    units: Option<&ResourceName>,
) -> Option<JsonNewMeasure> {
    if let Some(built_in) = built_in_measure(&measure.to_string()) {
        return Some(built_in);
    }
    let units = units.cloned().unwrap_or_else(JsonNewMeasure::generic_unit);
    match measure.clone() {
        NameId::Uuid(_) => None,
        NameId::Slug(slug) => Some(JsonNewMeasure {
            name: slug.clone().into(),
            slug: Some(slug),
            units,
        }),
        NameId::Name(name) => Some(JsonNewMeasure {
            name,
            slug: None,
            units,
        }),
    }
}

/// The built-in measure a measure name or slug names, if any.
fn built_in_measure(measure_str: &str) -> Option<JsonNewMeasure> {
    built_in::default::Latency::from_str(measure_str)
        .or_else(|| built_in::default::Throughput::from_str(measure_str))
        .or_else(|| built_in::default::BytesThroughput::from_str(measure_str))
        .or_else(|| built_in::default::Cycles::from_str(measure_str))
        .or_else(|| built_in::default::CpuTime::from_str(measure_str))
        .or_else(|| built_in::default::Allocations::from_str(measure_str))
        .or_else(|| built_in::dotnet::Gen0CollectsPer1000Ops::from_str(measure_str))
        .or_else(|| built_in::dotnet::Gen1CollectsPer1000Ops::from_str(measure_str))
        .or_else(|| built_in::dotnet::Gen2CollectsPer1000Ops::from_str(measure_str))
        .or_else(|| built_in::jmh::SampleTime::from_str(measure_str))
        .or_else(|| built_in::jmh::SingleShotTime::from_str(measure_str))
        .or_else(|| built_in::json::BuildTime::from_str(measure_str))
        .or_else(|| built_in::json::FileSize::from_str(measure_str))
        .or_else(|| built_in::iai::Instructions::from_str(measure_str))
        .or_else(|| built_in::iai::L1Accesses::from_str(measure_str))
        .or_else(|| built_in::iai::L2Accesses::from_str(measure_str))
        .or_else(|| built_in::iai::RamAccesses::from_str(measure_str))
        .or_else(|| built_in::iai::EstimatedCycles::from_str(measure_str))
        // noise
        .or_else(|| built_in::noise::ComputeJitter::from_str(measure_str))
        .or_else(|| built_in::noise::CacheJitter::from_str(measure_str))
        .or_else(|| built_in::noise::IoJitter::from_str(measure_str))
        .or_else(|| built_in::noise::CpuSteal::from_str(measure_str))
        .or_else(|| built_in::noise::NoiseScore::from_str(measure_str))
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
        .or_else(|| built_in::gungraun::ProfileTime::from_str(measure_str))
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

    pub fn into_query(self, id: MeasureId) -> QueryMeasure {
        let Self {
            uuid,
            project_id,
            name,
            slug,
            units,
            created,
            modified,
            archived,
        } = self;
        QueryMeasure {
            id,
            uuid,
            project_id,
            name,
            slug,
            units,
            created,
            modified,
            archived,
        }
    }

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

    fn units(units: ResourceName) -> Self {
        JsonUpdateMeasure {
            name: None,
            slug: None,
            units: Some(units),
            archived: None,
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    use bencher_json::DateTime;

    use super::MeasureId;
    use crate::{
        macros::sql::last_insert_rowid,
        schema,
        test_util::{create_base_entities, setup_test_db},
    };

    #[test]
    fn last_insert_rowid_returns_measure_id() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let uuid = "00000000-0000-0000-0000-000000000010";

        let (rowid, select_id) = conn
            .immediate_transaction(|conn| {
                diesel::insert_into(schema::measure::table)
                    .values((
                        schema::measure::uuid.eq(uuid),
                        schema::measure::project_id.eq(base.project_id),
                        schema::measure::name.eq("Measure 1"),
                        schema::measure::slug.eq("measure-1"),
                        schema::measure::units.eq("ns"),
                        schema::measure::created.eq(DateTime::TEST),
                        schema::measure::modified.eq(DateTime::TEST),
                    ))
                    .execute(conn)?;

                let rowid = diesel::select(last_insert_rowid()).get_result::<MeasureId>(conn)?;
                let select_id: MeasureId = schema::measure::table
                    .filter(schema::measure::uuid.eq(uuid))
                    .select(schema::measure::id)
                    .first(conn)?;

                diesel::QueryResult::Ok((rowid, select_id))
            })
            .expect("Transaction failed");

        assert_eq!(rowid, select_id);
    }

    #[test]
    fn last_insert_rowid_matches_second_measure() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        // Insert first
        diesel::insert_into(schema::measure::table)
            .values((
                schema::measure::uuid.eq("00000000-0000-0000-0000-000000000010"),
                schema::measure::project_id.eq(base.project_id),
                schema::measure::name.eq("Measure 1"),
                schema::measure::slug.eq("measure-1"),
                schema::measure::units.eq("ns"),
                schema::measure::created.eq(DateTime::TEST),
                schema::measure::modified.eq(DateTime::TEST),
            ))
            .execute(&mut conn)
            .expect("Failed to insert first measure");

        // Insert second + verify
        let second_uuid = "00000000-0000-0000-0000-000000000011";
        let (rowid, select_id) = conn
            .immediate_transaction(|conn| {
                diesel::insert_into(schema::measure::table)
                    .values((
                        schema::measure::uuid.eq(second_uuid),
                        schema::measure::project_id.eq(base.project_id),
                        schema::measure::name.eq("Measure 2"),
                        schema::measure::slug.eq("measure-2"),
                        schema::measure::units.eq("ms"),
                        schema::measure::created.eq(DateTime::TEST),
                        schema::measure::modified.eq(DateTime::TEST),
                    ))
                    .execute(conn)?;

                let rowid = diesel::select(last_insert_rowid()).get_result::<MeasureId>(conn)?;
                let select_id: MeasureId = schema::measure::table
                    .filter(schema::measure::uuid.eq(second_uuid))
                    .select(schema::measure::id)
                    .first(conn)?;

                diesel::QueryResult::Ok((rowid, select_id))
            })
            .expect("Transaction failed");

        assert_eq!(rowid, select_id);

        let first_id: MeasureId = schema::measure::table
            .filter(schema::measure::uuid.eq("00000000-0000-0000-0000-000000000010"))
            .select(schema::measure::id)
            .first(&mut conn)
            .expect("Failed to get first measure id");
        assert_ne!(rowid, first_id);
    }

    #[test]
    fn measure_insert_and_readback_same_conn() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let uuid = "00000000-0000-0000-0000-000000000010";

        // Insert and read back within same transaction
        let (inserted_id, readback_name) = conn
            .immediate_transaction(|conn| {
                diesel::insert_into(schema::measure::table)
                    .values((
                        schema::measure::uuid.eq(uuid),
                        schema::measure::project_id.eq(base.project_id),
                        schema::measure::name.eq("Test Measure"),
                        schema::measure::slug.eq("test-measure"),
                        schema::measure::units.eq("ns"),
                        schema::measure::created.eq(DateTime::TEST),
                        schema::measure::modified.eq(DateTime::TEST),
                    ))
                    .execute(conn)?;

                let id = diesel::select(last_insert_rowid()).get_result::<MeasureId>(conn)?;
                let name: String = schema::measure::table
                    .filter(schema::measure::uuid.eq(uuid))
                    .select(schema::measure::name)
                    .first(conn)?;

                diesel::QueryResult::Ok((id, name))
            })
            .expect("Transaction failed");

        assert_eq!(readback_name, "Test Measure");

        // Verify outside transaction
        let outside_id: MeasureId = schema::measure::table
            .filter(schema::measure::uuid.eq(uuid))
            .select(schema::measure::id)
            .first(&mut conn)
            .expect("Failed to read back");
        assert_eq!(inserted_id, outside_id);
    }
}
