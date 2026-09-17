use bencher_json::{
    DateTime, JsonVariant, ParameterSet, VariantUuid,
    project::{report::JsonReportVariant, variant::JsonUpdateVariant},
};
use diesel::{
    ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _,
    SelectableHelper as _,
};
use dropshot::HttpError;

use crate::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::{BencherResource, assert_parentage, issue_error, resource_conflict_err},
    macros::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        sql::last_insert_rowid,
    },
    schema::{self, variant as variant_table},
    write_conn, write_transaction,
};

use super::{
    ProjectId, QueryProject,
    benchmark::{BenchmarkId, QueryBenchmark},
};

crate::macros::typed_id::typed_id!(VariantId);

/// A variant: a benchmark with one set of parameters.
///
/// Variants have neither a name nor a slug, so they are UUID addressed only,
/// following the `report` and `alert` precedent.
#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = variant_table)]
#[diesel(belongs_to(QueryBenchmark, foreign_key = benchmark_id))]
pub struct QueryVariant {
    pub id: VariantId,
    pub uuid: VariantUuid,
    pub benchmark_id: BenchmarkId,
    pub parameters: ParameterSet,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl QueryVariant {
    fn_get!(variant, VariantId);
    fn_get_id!(variant, VariantId, VariantUuid);
    fn_get_uuid!(variant, VariantId, VariantUuid);
    fn_from_uuid!(benchmark_id, BenchmarkId, variant, VariantUuid, Variant);

    /// Get the benchmark's empty variant.
    ///
    /// Every benchmark is created atomically with its empty variant,
    /// so a missing row is data corruption and not a missing get-or-create.
    pub fn get_empty_id(
        conn: &mut DbConnection,
        benchmark_id: BenchmarkId,
    ) -> Result<VariantId, HttpError> {
        schema::variant::table
            .filter(schema::variant::benchmark_id.eq(benchmark_id))
            .filter(schema::variant::parameters.eq(ParameterSet::default()))
            .select(schema::variant::id)
            .first(conn)
            .map_err(|e| {
                let message =
                    format!("Failed to query the empty variant for benchmark ({benchmark_id})");
                issue_error(&message, &message, e)
            })
    }

    /// Resolve a reported variant to its row, creating it if it is new.
    ///
    /// The empty variant is never created here: every benchmark is born with
    /// one, so its absence is data corruption rather than a row to mint. Every
    /// other variant is created on first sight by its canonical form, and
    /// `UNIQUE(benchmark_id, parameters)` is what makes that idempotent under
    /// concurrent reports.
    ///
    /// Mirrors [`QueryBenchmark::get_or_create`]: a resolved variant that is archived is
    /// unarchived, because a variant that reports again is a live variant.
    ///
    /// Called from report ingest's read phase, so the write transaction it opens
    /// never nests inside the ingest write transaction.
    pub async fn get_or_create(
        context: &ApiContext,
        project_id: ProjectId,
        benchmark_id: BenchmarkId,
        parameters: &ParameterSet,
    ) -> Result<VariantId, HttpError> {
        let query_variant =
            Self::get_or_create_inner(context, project_id, benchmark_id, parameters).await?;

        if query_variant.archived.is_some() {
            let update_variant = UpdateVariant::unarchive();
            diesel::update(schema::variant::table.filter(schema::variant::id.eq(query_variant.id)))
                .set(&update_variant)
                .execute(write_conn!(context))
                .map_err(resource_conflict_err!(Variant, &query_variant))?;
        }

        Ok(query_variant.id)
    }

    async fn get_or_create_inner(
        context: &ApiContext,
        project_id: ProjectId,
        benchmark_id: BenchmarkId,
        parameters: &ParameterSet,
    ) -> Result<Self, HttpError> {
        if let Some(query_variant) =
            Self::from_parameters(auth_conn!(context), benchmark_id, parameters)?
        {
            return Ok(query_variant);
        }

        if parameters.is_empty() {
            let message = format!("Benchmark ({benchmark_id}) has no empty variant to report to");
            return Err(issue_error(
                "Failed to find the empty variant",
                &message,
                diesel::result::Error::NotFound,
            ));
        }

        match Self::create(context, project_id, benchmark_id, parameters).await {
            Ok(query_variant) => Ok(query_variant),
            Err(e) if crate::error::is_conflict(&e) => {
                // Another concurrent report created this variant.
                Self::from_parameters(auth_conn!(context), benchmark_id, parameters)?.ok_or(e)
            },
            Err(e) => Err(e),
        }
    }

    /// Create a variant under its benchmark.
    ///
    /// A variant that already exists under the benchmark collides on
    /// `UNIQUE(benchmark_id, parameters)`, so this is create and not get-or-create.
    /// The per project ceiling is checked first, exactly as it is for a variant a
    /// report mints.
    pub async fn create(
        context: &ApiContext,
        project_id: ProjectId,
        benchmark_id: BenchmarkId,
        parameters: &ParameterSet,
    ) -> Result<Self, HttpError> {
        #[cfg(feature = "plus")]
        InsertVariant::rate_limit(context, project_id).await?;
        #[cfg(not(feature = "plus"))]
        let _ = project_id;

        let insert_variant = InsertVariant::new(benchmark_id, parameters.clone(), DateTime::now());

        write_transaction!(context, |conn| {
            diesel::insert_into(schema::variant::table)
                .values(&insert_variant)
                .execute(conn)?;
            diesel::select(last_insert_rowid()).get_result::<VariantId>(conn)
        })
        .map_err(resource_conflict_err!(Variant, &insert_variant))
        .map(|id| insert_variant.into_query(id))
    }

    fn from_parameters(
        conn: &mut DbConnection,
        benchmark_id: BenchmarkId,
        parameters: &ParameterSet,
    ) -> Result<Option<Self>, HttpError> {
        schema::variant::table
            .filter(schema::variant::benchmark_id.eq(benchmark_id))
            .filter(schema::variant::parameters.eq(parameters))
            .select(Self::as_select())
            .first(conn)
            .optional()
            .map_err(|e| {
                let message = format!(
                    "Failed to query variant ({parameters}) for benchmark ({benchmark_id})"
                );
                issue_error(&message, &message, e)
            })
    }

    /// The variant as its own resource, under its benchmark.
    pub fn into_json_for_benchmark(self, benchmark: &QueryBenchmark) -> JsonVariant {
        let Self {
            id: _,
            uuid,
            benchmark_id,
            parameters,
            created,
            modified,
            archived,
        } = self;
        assert_parentage(
            BencherResource::Benchmark,
            benchmark.id,
            BencherResource::Variant,
            benchmark_id,
        );
        JsonVariant {
            uuid,
            benchmark: benchmark.uuid,
            parameters,
            created,
            modified,
            archived,
        }
    }

    /// The variant as a report result names it.
    pub fn into_report_json(self) -> JsonReportVariant {
        let Self {
            id: _,
            uuid,
            benchmark_id: _,
            parameters,
            created: _,
            modified: _,
            archived: _,
        } = self;
        JsonReportVariant { uuid, parameters }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = variant_table)]
pub struct InsertVariant {
    pub uuid: VariantUuid,
    pub benchmark_id: BenchmarkId,
    pub parameters: ParameterSet,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl InsertVariant {
    /// The per project ceiling on minting variants.
    ///
    /// Hand written rather than [`fn_rate_limit`](crate::macros::rate_limit) because
    /// `variant` has no `project_id` of its own: a variant belongs to its
    /// benchmark, and the benchmark is what belongs to the project. The window is
    /// counted through that join instead, with the same limits and the same error as
    /// every other resource a report mints.
    ///
    /// The count includes the empty variant each benchmark is born with, which
    /// makes the ceiling slightly conservative for a project creating benchmarks and
    /// variants in the same window. That is the safe direction, and it costs a
    /// project nothing that the benchmark ceiling was not already going to cost it.
    #[cfg(feature = "plus")]
    async fn rate_limit(context: &ApiContext, project_id: ProjectId) -> Result<(), HttpError> {
        use crate::error::BencherResource;

        let query_project = QueryProject::get(auth_conn!(context), project_id)?;
        let query_organization = query_project.organization(auth_conn!(context))?;
        let is_claimed = query_organization.is_claimed(auth_conn!(context))?;

        let (start_time, end_time) = context.rate_limiting.window();
        let window_usage: u32 = schema::variant::table
            .inner_join(schema::benchmark::table)
            .filter(schema::benchmark::project_id.eq(project_id))
            .filter(schema::variant::created.ge(start_time))
            .filter(schema::variant::created.le(end_time))
            .count()
            .get_result::<i64>(auth_conn!(context))
            .map_err(crate::error::resource_not_found_err!(
                Variant,
                (project_id, start_time, end_time)
            ))?
            .try_into()
            .map_err(|e| {
                issue_error(
                    "Failed to count creation",
                    &format!(
                        "Failed to count variant creation for project ({project_id}) between {start_time} and {end_time}."
                    ),
                    e,
                )
            })?;

        context.rate_limiting.check_claimable_limit(
            is_claimed,
            window_usage,
            |rate_limit| crate::context::RateLimitingError::UnclaimedProject {
                project: query_project.clone(),
                resource: BencherResource::Variant,
                rate_limit,
            },
            |rate_limit| crate::context::RateLimitingError::ClaimedProject {
                project: query_project.clone(),
                resource: BencherResource::Variant,
                rate_limit,
            },
        )
    }

    /// The empty variant that every benchmark is born with.
    ///
    /// The timestamp is the benchmark's own creation timestamp:
    /// the variant is created in the same transaction as its benchmark.
    pub fn empty(benchmark_id: BenchmarkId, timestamp: DateTime) -> Self {
        Self::new(benchmark_id, ParameterSet::default(), timestamp)
    }

    /// A variant as a report first named it, already canonical.
    pub fn new(benchmark_id: BenchmarkId, parameters: ParameterSet, timestamp: DateTime) -> Self {
        Self {
            uuid: VariantUuid::new(),
            benchmark_id,
            parameters,
            created: timestamp,
            modified: timestamp,
            archived: None,
        }
    }

    pub fn into_query(self, id: VariantId) -> QueryVariant {
        let Self {
            uuid,
            benchmark_id,
            parameters,
            created,
            modified,
            archived,
        } = self;
        QueryVariant {
            id,
            uuid,
            benchmark_id,
            parameters,
            created,
            modified,
            archived,
        }
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = variant_table)]
pub struct UpdateVariant {
    pub modified: DateTime,
    pub archived: Option<Option<DateTime>>,
}

impl From<JsonUpdateVariant> for UpdateVariant {
    fn from(update: JsonUpdateVariant) -> Self {
        let JsonUpdateVariant { archived } = update;
        let modified = DateTime::now();
        let archived = archived.map(|archived| archived.then_some(modified));
        Self { modified, archived }
    }
}

impl UpdateVariant {
    /// A variant that reports again is a live variant.
    fn unarchive() -> Self {
        Self {
            modified: DateTime::now(),
            archived: Some(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, ParameterSet, VariantUuid};
    use diesel::{
        ExpressionMethods as _, QueryDsl as _, QueryResult, RunQueryDsl as _, SqliteConnection,
        connection::SimpleConnection as _,
    };
    use diesel_migrations::MigrationHarness as _;

    use bencher_json::project::parameter::jsonb;

    use crate::{
        model::project::benchmark::BenchmarkId,
        schema,
        test_util::{
            create_base_entities, create_benchmark, create_branch_with_head, create_report,
            create_report_benchmark, create_testbed, create_variant, create_version,
            get_empty_variant, setup_test_db,
        },
    };

    /// Where the blob under test comes from.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Encode {
        /// Written through the `variant.parameters` column: the production path.
        Column,
        /// Encoded directly. Parameter values are scalar only, so a null value
        /// never reaches the column, but the encoder still has to agree with `SQLite`.
        Encoder,
    }

    /// Every parameter set that has to encode to the same bytes as `SQLite`'s `jsonb()`.
    ///
    /// Each entry is already in its RFC 8785 (JCS) canonical form. The set covers
    /// the shapes a parameter value can take: strings that need JSON escapes and
    /// strings that do not, exponent form floats, integers above `i64`, control and
    /// supplementary plane characters, and key orders where the UTF-16 sort differs
    /// from the UTF-8 one.
    const CONFORMANCE: &[(&str, &str, Encode)] = &[
        ("empty", "{}", Encode::Column),
        (
            "realistic",
            r#"{"label":"say \"hi\"","path":"C:\\bench\\x","tolerance":1e-7}"#,
            Encode::Column,
        ),
        (
            "scalars",
            r#"{"debug":true,"os":"linux","threads":4}"#,
            Encode::Column,
        ),
        ("null", r#"{"x":null}"#, Encode::Encoder),
        ("float-simple", r#"{"a":0.1,"z":16.5}"#, Encode::Column),
        ("float-tiny", r#"{"b":1e-7}"#, Encode::Column),
        ("float-huge", r#"{"c":1e+21}"#, Encode::Column),
        ("float-min-sub", r#"{"d":5e-324}"#, Encode::Column),
        (
            "float-max",
            r#"{"e":1.7976931348623157e+308}"#,
            Encode::Column,
        ),
        ("big-int", r#"{"n":10000000000000000000}"#, Encode::Column),
        ("int-2p53", r#"{"n":9007199254740992}"#, Encode::Column),
        ("neg", r#"{"n":-1}"#, Encode::Column),
        ("str-quote", r#"{"q":"say \"hi\""}"#, Encode::Column),
        ("str-backslash", r#"{"s":"a\\b"}"#, Encode::Column),
        ("str-newline", r#"{"s":"x\ny"}"#, Encode::Column),
        ("str-tab", r#"{"s":"a\tb"}"#, Encode::Column),
        ("str-del", "{\"s\":\"a\u{7f}b\"}", Encode::Column),
        ("str-unicode", r#"{"s":"héllo"}"#, Encode::Column),
        (
            "nonbmp-keys",
            "{\"\u{1f600}\":1,\"\u{fb33}\":2}",
            Encode::Column,
        ),
    ];

    #[derive(diesel::QueryableByName)]
    struct SqlText {
        #[diesel(sql_type = diesel::sql_types::Text)]
        value: String,
    }

    #[derive(diesel::QueryableByName)]
    struct SqlInteger {
        #[diesel(sql_type = diesel::sql_types::Integer)]
        value: i32,
    }

    fn parameters(parameters: &str) -> ParameterSet {
        parameters.parse().expect("Failed to parse parameters")
    }

    fn hex(blob: &[u8]) -> String {
        use std::fmt::Write as _;

        blob.iter().fold(String::new(), |mut hex, byte| {
            write!(hex, "{byte:02X}").expect("Failed to format a byte");
            hex
        })
    }

    /// The bytes `SQLite`'s own `jsonb()` produces for a canonical text.
    fn sqlite_jsonb(conn: &mut SqliteConnection, canonical: &str) -> String {
        diesel::sql_query("SELECT hex(jsonb(?)) AS value")
            .bind::<diesel::sql_types::Text, _>(canonical)
            .get_result::<SqlText>(conn)
            .expect("Failed to mint a JSONB blob")
            .value
    }

    /// A scalar SQL expression over one stored variant.
    fn variant_text(
        conn: &mut SqliteConnection,
        variant_id: super::VariantId,
        sql: &str,
    ) -> String {
        diesel::sql_query(format!("SELECT {sql} AS value FROM variant WHERE id = ?"))
            .bind::<diesel::sql_types::Integer, _>(variant_id)
            .get_result::<SqlText>(conn)
            .expect("Failed to read the variant")
            .value
    }

    fn variant_integer(
        conn: &mut SqliteConnection,
        variant_id: super::VariantId,
        sql: &str,
    ) -> i32 {
        diesel::sql_query(format!("SELECT {sql} AS value FROM variant WHERE id = ?"))
            .bind::<diesel::sql_types::Integer, _>(variant_id)
            .get_result::<SqlInteger>(conn)
            .expect("Failed to read the variant")
            .value
    }

    /// A scalar SQL expression over a blob bound directly, for encoded bytes that
    /// never reach the column.
    fn blob_text(conn: &mut SqliteConnection, sql: &str, blob: Vec<u8>) -> String {
        diesel::sql_query(format!("SELECT {sql} AS value"))
            .bind::<diesel::sql_types::Binary, _>(blob)
            .get_result::<SqlText>(conn)
            .expect("Failed to read the encoded variant")
            .value
    }

    fn blob_integer(conn: &mut SqliteConnection, sql: &str, blob: Vec<u8>) -> i32 {
        diesel::sql_query(format!("SELECT {sql} AS value"))
            .bind::<diesel::sql_types::Binary, _>(blob)
            .get_result::<SqlInteger>(conn)
            .expect("Failed to read the encoded variant")
            .value
    }

    /// Mint a variant with `SQLite`'s `jsonb()`, the migration's write path.
    fn mint_variant(
        conn: &mut SqliteConnection,
        benchmark_id: BenchmarkId,
        canonical: &str,
    ) -> QueryResult<usize> {
        diesel::sql_query(
            "INSERT INTO variant(uuid, benchmark_id, parameters, created, modified)
               VALUES (?, ?, jsonb(?), 0, 0)",
        )
        .bind::<diesel::sql_types::Text, _>(VariantUuid::new().to_string())
        .bind::<diesel::sql_types::Integer, _>(benchmark_id)
        .bind::<diesel::sql_types::Text, _>(canonical)
        .execute(conn)
    }

    fn is_unique_violation(result: &QueryResult<usize>) -> bool {
        matches!(
            result,
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _
            ))
        )
    }

    /// Whether a write either landed or collided on the unique constraint,
    /// as opposed to failing for some other reason.
    fn landed_or_collided(result: &QueryResult<usize>) -> bool {
        match result {
            Ok(_) => true,
            Err(_) => is_unique_violation(result),
        }
    }

    /// The variant a benchmark was born with, or a freshly written one.
    fn write_variant(
        conn: &mut SqliteConnection,
        benchmark_id: BenchmarkId,
        parameters: &ParameterSet,
    ) -> super::VariantId {
        if parameters.is_empty() {
            get_empty_variant(conn, benchmark_id)
        } else {
            create_variant(conn, benchmark_id, parameters)
        }
    }

    // The encoder has to be byte identical to SQLite's `jsonb()` over the same
    // canonical text, because `UNIQUE(benchmark_id, parameters)` compares bytes and
    // both writers reach that column: the migration mints the empty variant with
    // `jsonb('{}')` and everything after that is written through Diesel.
    #[test]
    fn byte_agreement_with_sqlite_jsonb() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let benchmark_id = create_benchmark(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "bench1",
            "bench1",
        );

        for (name, canonical, encode) in CONFORMANCE {
            let minted = sqlite_jsonb(&mut conn, canonical);
            match encode {
                Encode::Column => {
                    let parameters = parameters(canonical);
                    assert_eq!(
                        parameters.canonical(),
                        *canonical,
                        "{name}: the conformance text is already canonical"
                    );

                    let variant_id = write_variant(&mut conn, benchmark_id, &parameters);
                    assert_eq!(
                        variant_text(&mut conn, variant_id, "hex(parameters)"),
                        minted,
                        "{name}: the written bytes must be the bytes jsonb() mints"
                    );
                    assert_eq!(
                        variant_integer(&mut conn, variant_id, "json_valid(parameters, 8)"),
                        1,
                        "{name}: SQLite's JSON functions must accept the written bytes"
                    );
                    assert_eq!(
                        variant_text(&mut conn, variant_id, "json(parameters)"),
                        *canonical,
                        "{name}: the canonical text must survive the column unchanged"
                    );
                },
                Encode::Encoder => {
                    // `{"x":null}`. Scalar only validation rejects a null value, so
                    // this one set is encoded directly rather than through the column.
                    // The column write, the unique collision and the `FromSql` round
                    // trip are the only assertions that need the column, so the bytes
                    // are bound directly and still checked against `jsonb()`,
                    // `json_valid()` and `json()`.
                    let mut object = jsonb::Object::default();
                    object
                        .insert_null("x")
                        .expect("Failed to encode a null member");
                    let blob = object.into_blob().expect("Failed to encode the object");
                    assert_eq!(
                        hex(&blob),
                        minted,
                        "{name}: the encoded bytes must be the bytes jsonb() mints"
                    );
                    assert_eq!(
                        blob_integer(&mut conn, "json_valid(?, 8)", blob.clone()),
                        1,
                        "{name}: SQLite's JSON functions must accept the encoded bytes"
                    );
                    assert_eq!(
                        blob_text(&mut conn, "json(?)", blob),
                        *canonical,
                        "{name}: the canonical text must survive the encoder unchanged"
                    );
                },
            }
        }
    }

    // Parameters read back out of the column have to be the ones that were
    // written, whichever writer wrote it, and re-canonicalizing them has to land on
    // the same text or the unique constraint stops holding.
    #[test]
    fn parameters_read_back_from_both_writers() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let written = create_benchmark(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "bench1",
            "bench1",
        );
        let minted = create_benchmark(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000011",
            "bench2",
            "bench2",
        );
        // SQLite mints every variant under this benchmark, the empty one included, so
        // the Diesel written variant it was born with is cleared out of the way first.
        diesel::delete(schema::variant::table.filter(schema::variant::benchmark_id.eq(minted)))
            .execute(&mut conn)
            .expect("Failed to clear the minted benchmark's variants");

        for (name, canonical, encode) in CONFORMANCE {
            if *encode != Encode::Column {
                continue;
            }
            let parameters = parameters(canonical);

            let variant_id = write_variant(&mut conn, written, &parameters);
            let read: ParameterSet = schema::variant::table
                .filter(schema::variant::id.eq(variant_id))
                .select(schema::variant::parameters)
                .first(&mut conn)
                .expect("Failed to read back a written variant");
            assert_eq!(read, parameters, "{name}: written and read back");
            assert_eq!(
                read.canonical(),
                *canonical,
                "{name}: written stays canonical"
            );

            mint_variant(&mut conn, minted, canonical).expect("Failed to mint a variant");
            let read: ParameterSet = schema::variant::table
                .filter(schema::variant::benchmark_id.eq(minted))
                .order(schema::variant::id.desc())
                .select(schema::variant::parameters)
                .first(&mut conn)
                .expect("Failed to read back a minted variant");
            assert_eq!(read, parameters, "{name}: minted and read back");
            assert_eq!(
                read.canonical(),
                *canonical,
                "{name}: minted stays canonical"
            );
        }
    }

    // The unique constraint is the enforcement point, so a set written through
    // Diesel and the same set minted by `jsonb()` have to collide on it.
    #[test]
    fn write_paths_collide_on_unique() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        for (index, (name, canonical, encode)) in CONFORMANCE.iter().enumerate() {
            if *encode != Encode::Column {
                continue;
            }
            let benchmark_id = create_benchmark(
                &mut conn,
                base.project_id,
                &format!("00000000-0000-0000-0000-{index:012}"),
                &format!("bench{index}"),
                &format!("bench{index}"),
            );

            // The empty variant is already there, written through Diesel when the
            // benchmark was born, so for that one the mint is what collides.
            let minted = mint_variant(&mut conn, benchmark_id, canonical);
            let written = insert_variant(&mut conn, benchmark_id, &parameters(canonical));

            assert!(
                is_unique_violation(&minted) || is_unique_violation(&written),
                "{name}: the two writers must collide on UNIQUE(benchmark_id, parameters)"
            );
            assert!(
                landed_or_collided(&minted),
                "{name}: the mint must either land or collide"
            );
            assert!(
                landed_or_collided(&written),
                "{name}: the write must either land or collide"
            );

            let expected = if *canonical == "{}" { 1 } else { 2 };
            assert_eq!(
                count_variants(&mut conn, benchmark_id),
                expected,
                "{name}: one row per distinct variant"
            );
        }
    }

    fn insert_variant(
        conn: &mut SqliteConnection,
        benchmark_id: BenchmarkId,
        parameters: &ParameterSet,
    ) -> QueryResult<usize> {
        diesel::insert_into(schema::variant::table)
            .values((
                schema::variant::uuid.eq(VariantUuid::new()),
                schema::variant::benchmark_id.eq(benchmark_id),
                schema::variant::parameters.eq(parameters),
                schema::variant::created.eq(DateTime::TEST),
                schema::variant::modified.eq(DateTime::TEST),
            ))
            .execute(conn)
    }

    fn count_variants(conn: &mut SqliteConnection, benchmark_id: BenchmarkId) -> i64 {
        schema::variant::table
            .filter(schema::variant::benchmark_id.eq(benchmark_id))
            .count()
            .get_result(conn)
            .expect("Failed to count parameters")
    }

    #[test]
    fn key_order_collides_on_unique() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let benchmark_id = create_benchmark(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "bench1",
            "bench1",
        );

        insert_variant(&mut conn, benchmark_id, &parameters(r#"{"b": 1, "a": 2}"#))
            .expect("Failed to insert variant");
        let collision = insert_variant(&mut conn, benchmark_id, &parameters(r#"{"a": 2, "b": 1}"#));

        assert!(
            collision.is_err(),
            "logically equal variants must collide on UNIQUE(benchmark_id, parameters)"
        );
        // The empty variant the benchmark was born with, plus the one that landed.
        assert_eq!(count_variants(&mut conn, benchmark_id), 2);
    }

    #[test]
    fn number_spelling_collides_on_unique() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let benchmark_id = create_benchmark(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "bench1",
            "bench1",
        );

        insert_variant(&mut conn, benchmark_id, &parameters(r#"{"n": 16}"#))
            .expect("Failed to insert variant");
        for spelling in [r#"{"n": 16.0}"#, r#"{"n": 1.6e1}"#] {
            assert!(
                insert_variant(&mut conn, benchmark_id, &parameters(spelling)).is_err(),
                "{spelling} must collide with 16"
            );
        }

        assert_eq!(count_variants(&mut conn, benchmark_id), 2);
    }

    #[test]
    fn identical_parameters_under_distinct_benchmarks() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let first = create_benchmark(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "bench1",
            "bench1",
        );
        let second = create_benchmark(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000011",
            "bench2",
            "bench2",
        );

        let variant = parameters(r#"{"size_mb": 16}"#);
        insert_variant(&mut conn, first, &variant).expect("Failed to insert variant");
        insert_variant(&mut conn, second, &variant).expect("Failed to insert variant");

        assert_eq!(count_variants(&mut conn, first), 2);
        assert_eq!(count_variants(&mut conn, second), 2);
    }

    /// Seed the pre-migration shape: benchmarks and `report_benchmark` rows with
    /// no `variant_id`, written as raw SQL because the Diesel DSL describes the
    /// post-migration schema.
    fn seed_legacy_rows(conn: &mut SqliteConnection) {
        conn.batch_execute(
            "INSERT INTO organization (uuid, name, slug, created, modified)
                VALUES ('00000000-0000-0000-0000-000000000001', 'Org', 'org', 0, 0);
            INSERT INTO project (uuid, organization_id, name, slug, visibility, created, modified)
                VALUES ('00000000-0000-0000-0000-000000000002', 1, 'Project', 'project', 0, 0, 0);
            INSERT INTO branch (uuid, project_id, name, slug, created, modified)
                VALUES ('00000000-0000-0000-0000-000000000003', 1, 'main', 'main', 0, 0);
            INSERT INTO head (uuid, branch_id, created)
                VALUES ('00000000-0000-0000-0000-000000000004', 1, 0);
            UPDATE branch SET head_id = 1 WHERE id = 1;
            INSERT INTO version (uuid, project_id, number)
                VALUES ('00000000-0000-0000-0000-000000000005', 1, 1);
            INSERT INTO head_version (head_id, version_id) VALUES (1, 1);
            INSERT INTO testbed (uuid, project_id, name, slug, created, modified)
                VALUES ('00000000-0000-0000-0000-000000000006', 1, 'localhost', 'localhost', 0, 0);
            INSERT INTO report (uuid, project_id, head_id, version_id, testbed_id, adapter, start_time, end_time, created)
                VALUES ('00000000-0000-0000-0000-000000000007', 1, 1, 1, 1, 0, 0, 0, 0);
            INSERT INTO benchmark (uuid, project_id, name, slug, created, modified)
                VALUES ('00000000-0000-0000-0000-000000000008', 1, 'bench1', 'bench1', 0, 0);
            INSERT INTO benchmark (uuid, project_id, name, slug, created, modified)
                VALUES ('00000000-0000-0000-0000-000000000009', 1, 'bench2', 'bench2', 0, 0);
            INSERT INTO report_benchmark (uuid, report_id, iteration, benchmark_id)
                VALUES ('00000000-0000-0000-0000-000000000010', 1, 0, 1);
            INSERT INTO report_benchmark (uuid, report_id, iteration, benchmark_id)
                VALUES ('00000000-0000-0000-0000-000000000011', 1, 1, 1);
            INSERT INTO report_benchmark (uuid, report_id, iteration, benchmark_id)
                VALUES ('00000000-0000-0000-0000-000000000012', 1, 0, 2);",
        )
        .expect("Failed to seed legacy rows");
    }

    /// Revert every migration down to and including the parameter migration.
    ///
    /// The parameter migration is not the last one any more, so reverting only the
    /// last one would revert someone else's. Each layer above it is reverted first,
    /// and `run_pending_migrations` puts them all back.
    fn revert_to_parameter_migration(conn: &mut SqliteConnection) {
        const PARAMETER_MIGRATION: &str = "20260815120000";

        loop {
            let version = conn
                .revert_last_migration(crate::MIGRATIONS)
                .expect("Failed to revert a migration");
            if version.to_string() == PARAMETER_MIGRATION {
                break;
            }
        }
    }

    #[test]
    fn migration_backfills_empty_variants() {
        let mut conn = setup_test_db();

        // Foreign keys cannot be toggled inside a transaction, and Diesel runs each
        // migration in one, so they are disabled around the revert and re-apply.
        conn.batch_execute("PRAGMA foreign_keys = OFF")
            .expect("Failed to disable foreign keys");
        revert_to_parameter_migration(&mut conn);

        seed_legacy_rows(&mut conn);

        conn.run_pending_migrations(crate::MIGRATIONS)
            .expect("Failed to re-apply the parameter migration");
        conn.batch_execute("PRAGMA foreign_keys = ON")
            .expect("Failed to enable foreign keys");

        let benchmark_ids: Vec<BenchmarkId> = schema::benchmark::table
            .order(schema::benchmark::id.asc())
            .select(schema::benchmark::id)
            .load(&mut conn)
            .expect("Failed to load benchmarks");
        assert_eq!(benchmark_ids.len(), 2);

        for benchmark_id in benchmark_ids {
            let backfilled: Vec<ParameterSet> = schema::variant::table
                .filter(schema::variant::benchmark_id.eq(benchmark_id))
                .select(schema::variant::parameters)
                .load(&mut conn)
                .expect("Failed to load parameters");
            assert_eq!(
                backfilled,
                vec![ParameterSet::default()],
                "every benchmark gets exactly one empty variant"
            );

            // The migration mints the canonical empty object in SQL, so a set minted
            // in Rust has to be byte identical to it.
            assert!(
                insert_variant(&mut conn, benchmark_id, &ParameterSet::default()).is_err(),
                "the backfilled empty variant must collide with a Rust minted one"
            );
        }

        let report_benchmarks: Vec<(BenchmarkId, super::VariantId)> =
            schema::report_benchmark::table
                .select((
                    schema::report_benchmark::benchmark_id,
                    schema::report_benchmark::variant_id,
                ))
                .load(&mut conn)
                .expect("Failed to load report benchmarks");
        assert_eq!(report_benchmarks.len(), 3);
        for (benchmark_id, variant_id) in report_benchmarks {
            let empty_variant_id = super::QueryVariant::get_empty_id(&mut conn, benchmark_id)
                .expect("Failed to get the empty variant");
            assert_eq!(
                variant_id, empty_variant_id,
                "every report benchmark points at its own benchmark's empty variant"
            );
        }
    }

    /// The indexes `report_benchmark` carries, in name order.
    fn report_benchmark_indexes(conn: &mut SqliteConnection) -> Vec<String> {
        diesel::sql_query(
            "SELECT name AS value FROM sqlite_master
                WHERE type = 'index' AND tbl_name = 'report_benchmark'
                ORDER BY name",
        )
        .load::<SqlText>(conn)
        .expect("Failed to read the report benchmark indexes")
        .into_iter()
        .map(|index| index.value)
        .collect()
    }

    /// Re-insert an existing `report_benchmark` row under a new uuid, which collides
    /// on the unique key over the report, iteration, benchmark, and variant.
    fn duplicate_report_benchmark_key(conn: &mut SqliteConnection) -> QueryResult<usize> {
        diesel::sql_query(
            "INSERT INTO report_benchmark (uuid, report_id, iteration, benchmark_id, variant_id)
                SELECT '00000000-0000-0000-0000-000000000013',
                    report_id, iteration, benchmark_id, variant_id
                FROM report_benchmark
                WHERE id = 1",
        )
        .execute(conn)
    }

    /// Re-insert an existing `report_benchmark` row under a new iteration, which
    /// collides on the unique uuid.
    fn duplicate_report_benchmark_uuid(conn: &mut SqliteConnection) -> QueryResult<usize> {
        diesel::sql_query(
            "INSERT INTO report_benchmark (uuid, report_id, iteration, benchmark_id, variant_id)
                SELECT uuid, report_id, 42, benchmark_id, variant_id
                FROM report_benchmark
                WHERE id = 1",
        )
        .execute(conn)
    }

    /// Seed one `report_benchmark` row, so that a second row can be made to
    /// collide with it.
    fn seed_report_benchmark(conn: &mut SqliteConnection) {
        let uuid = |n: u8| format!("00000000-0000-0000-0000-0000000000{n:02x}");
        let base = create_base_entities(conn);
        let branch =
            create_branch_with_head(conn, base.project_id, &uuid(3), "Main", "main", &uuid(4));
        let version = create_version(conn, base.project_id, &uuid(5), 1, None);
        let testbed = create_testbed(conn, base.project_id, &uuid(6), "Testbed", "testbed");
        let report = create_report(
            conn,
            &uuid(7),
            base.project_id,
            branch.head_id,
            version,
            testbed,
        );
        let benchmark = create_benchmark(conn, base.project_id, &uuid(8), "Bench", "bench");
        create_report_benchmark(conn, &uuid(9), report, 0, benchmark);
    }

    // The migration builds `report_benchmark`'s unique indexes after the backfill
    // rather than declaring them on the table, so both are named indexes rather than
    // implicit autoindexes. What they enforce is unchanged, which is the other half
    // of this: a repeated key and a repeated uuid each still collide.
    #[test]
    fn migration_defers_the_report_benchmark_unique_indexes() {
        let mut conn = setup_test_db();

        seed_report_benchmark(&mut conn);

        assert_eq!(
            report_benchmark_indexes(&mut conn),
            vec![
                "index_report_benchmark_benchmark_report".to_owned(),
                "index_report_benchmark_report_iteration_benchmark_variant".to_owned(),
                "index_report_benchmark_uuid".to_owned(),
                "index_report_benchmark_variant".to_owned(),
            ],
            "the rebuilt table's indexes are the named ones the migration builds"
        );

        let repeated_key = duplicate_report_benchmark_key(&mut conn);
        assert!(
            is_unique_violation(&repeated_key),
            "a repeated report, iteration, benchmark, and variant collides"
        );
        let repeated_uuid = duplicate_report_benchmark_uuid(&mut conn);
        assert!(
            is_unique_violation(&repeated_uuid),
            "a repeated uuid collides"
        );
    }

    #[test]
    fn migration_down_and_up_is_idempotent() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let benchmark_id = create_benchmark(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "bench1",
            "bench1",
        );
        assert_eq!(count_variants(&mut conn, benchmark_id), 1);

        conn.batch_execute("PRAGMA foreign_keys = OFF")
            .expect("Failed to disable foreign keys");
        revert_to_parameter_migration(&mut conn);
        conn.run_pending_migrations(crate::MIGRATIONS)
            .expect("Failed to re-apply the parameter migration");
        conn.batch_execute("PRAGMA foreign_keys = ON")
            .expect("Failed to enable foreign keys");

        assert_eq!(count_variants(&mut conn, benchmark_id), 1);
    }
}
