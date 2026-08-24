use bencher_json::{
    DateTime, JsonParameter, ParameterSet, ParameterUuid,
    project::{parameter::JsonUpdateParameter, report::JsonReportParameter},
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
    schema::{self, parameter as parameter_table},
    write_conn, write_transaction,
};

use super::{
    ProjectId, QueryProject,
    benchmark::{BenchmarkId, QueryBenchmark},
};

crate::macros::typed_id::typed_id!(ParameterId);

/// A parameter set: one grid point under its benchmark.
///
/// Parameter sets have neither a name nor a slug, so they are UUID addressed only,
/// following the `report` and `alert` precedent.
#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = parameter_table)]
#[diesel(belongs_to(QueryBenchmark, foreign_key = benchmark_id))]
pub struct QueryParameter {
    pub id: ParameterId,
    pub uuid: ParameterUuid,
    pub benchmark_id: BenchmarkId,
    pub set: ParameterSet,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl QueryParameter {
    fn_get!(parameter, ParameterId);
    fn_get_id!(parameter, ParameterId, ParameterUuid);
    fn_get_uuid!(parameter, ParameterId, ParameterUuid);
    fn_from_uuid!(
        benchmark_id,
        BenchmarkId,
        parameter,
        ParameterUuid,
        Parameter
    );

    /// Get the benchmark's empty parameter set.
    ///
    /// Every benchmark is created atomically with its empty parameter set,
    /// so a missing row is data corruption and not a missing get-or-create.
    pub fn get_empty_set_id(
        conn: &mut DbConnection,
        benchmark_id: BenchmarkId,
    ) -> Result<ParameterId, HttpError> {
        schema::parameter::table
            .filter(schema::parameter::benchmark_id.eq(benchmark_id))
            .filter(schema::parameter::set.eq(ParameterSet::default()))
            .select(schema::parameter::id)
            .first(conn)
            .map_err(|e| {
                let message = format!(
                    "Failed to query the empty parameter set for benchmark ({benchmark_id})"
                );
                issue_error(&message, &message, e)
            })
    }

    /// Resolve a reported parameter set to its row, creating it if it is new.
    ///
    /// The empty parameter set is never created here: every benchmark is born with
    /// one, so its absence is data corruption rather than a row to mint. Every
    /// other set is created on first sight by its canonical form, and
    /// `UNIQUE (benchmark_id, "set")` is what makes that idempotent under
    /// concurrent reports.
    ///
    /// Mirrors [`QueryBenchmark::get_or_create`]: a resolved set that is archived is
    /// unarchived, because a grid point that reports again is a live grid point.
    ///
    /// Called from report ingest's read phase, so the write transaction it opens
    /// never nests inside the ingest write transaction.
    pub async fn get_or_create(
        context: &ApiContext,
        project_id: ProjectId,
        benchmark_id: BenchmarkId,
        parameters: &ParameterSet,
    ) -> Result<ParameterId, HttpError> {
        let query_parameter =
            Self::get_or_create_inner(context, project_id, benchmark_id, parameters).await?;

        if query_parameter.archived.is_some() {
            let update_parameter = UpdateParameter::unarchive();
            diesel::update(
                schema::parameter::table.filter(schema::parameter::id.eq(query_parameter.id)),
            )
            .set(&update_parameter)
            .execute(write_conn!(context))
            .map_err(resource_conflict_err!(Parameter, &query_parameter))?;
        }

        Ok(query_parameter.id)
    }

    async fn get_or_create_inner(
        context: &ApiContext,
        project_id: ProjectId,
        benchmark_id: BenchmarkId,
        parameters: &ParameterSet,
    ) -> Result<Self, HttpError> {
        if let Some(query_parameter) =
            Self::from_parameters(auth_conn!(context), benchmark_id, parameters)?
        {
            return Ok(query_parameter);
        }

        if parameters.is_empty() {
            let message =
                format!("Benchmark ({benchmark_id}) has no empty parameter set to report to");
            return Err(issue_error(
                "Failed to find the empty parameter set",
                &message,
                diesel::result::Error::NotFound,
            ));
        }

        match Self::create(context, project_id, benchmark_id, parameters).await {
            Ok(query_parameter) => Ok(query_parameter),
            Err(e) if crate::error::is_conflict(&e) => {
                // Another concurrent report created this parameter set.
                Self::from_parameters(auth_conn!(context), benchmark_id, parameters)?.ok_or(e)
            },
            Err(e) => Err(e),
        }
    }

    /// Create a parameter set under its benchmark.
    ///
    /// A set that already exists under the benchmark collides on
    /// `UNIQUE (benchmark_id, "set")`, so this is create and not get-or-create.
    /// The per project ceiling is checked first, exactly as it is for a set a
    /// report mints.
    pub async fn create(
        context: &ApiContext,
        project_id: ProjectId,
        benchmark_id: BenchmarkId,
        parameters: &ParameterSet,
    ) -> Result<Self, HttpError> {
        #[cfg(feature = "plus")]
        InsertParameter::rate_limit(context, project_id).await?;
        #[cfg(not(feature = "plus"))]
        let _ = project_id;

        let insert_parameter =
            InsertParameter::new(benchmark_id, parameters.clone(), DateTime::now());

        write_transaction!(context, |conn| {
            diesel::insert_into(schema::parameter::table)
                .values(&insert_parameter)
                .execute(conn)?;
            diesel::select(last_insert_rowid()).get_result::<ParameterId>(conn)
        })
        .map_err(resource_conflict_err!(Parameter, &insert_parameter))
        .map(|id| insert_parameter.into_query(id))
    }

    fn from_parameters(
        conn: &mut DbConnection,
        benchmark_id: BenchmarkId,
        parameters: &ParameterSet,
    ) -> Result<Option<Self>, HttpError> {
        schema::parameter::table
            .filter(schema::parameter::benchmark_id.eq(benchmark_id))
            .filter(schema::parameter::set.eq(parameters))
            .select(Self::as_select())
            .first(conn)
            .optional()
            .map_err(|e| {
                let message = format!(
                    "Failed to query parameter set ({parameters}) for benchmark ({benchmark_id})"
                );
                issue_error(&message, &message, e)
            })
    }

    /// The parameter set as its own resource, under its benchmark.
    pub fn into_json_for_benchmark(self, benchmark: &QueryBenchmark) -> JsonParameter {
        let Self {
            id: _,
            uuid,
            benchmark_id,
            set,
            created,
            modified,
            archived,
        } = self;
        assert_parentage(
            BencherResource::Benchmark,
            benchmark.id,
            BencherResource::Parameter,
            benchmark_id,
        );
        JsonParameter {
            uuid,
            benchmark: benchmark.uuid,
            set,
            created,
            modified,
            archived,
        }
    }

    /// The parameter set as a report result names it.
    pub fn into_report_json(self) -> JsonReportParameter {
        let Self {
            id: _,
            uuid,
            benchmark_id: _,
            set,
            created: _,
            modified: _,
            archived: _,
        } = self;
        JsonReportParameter { uuid, set }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = parameter_table)]
pub struct InsertParameter {
    pub uuid: ParameterUuid,
    pub benchmark_id: BenchmarkId,
    pub set: ParameterSet,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl InsertParameter {
    /// The per project ceiling on minting parameter sets.
    ///
    /// Hand written rather than [`fn_rate_limit`](crate::macros::rate_limit) because
    /// `parameter` has no `project_id` of its own: a parameter set belongs to its
    /// benchmark, and the benchmark is what belongs to the project. The window is
    /// counted through that join instead, with the same limits and the same error as
    /// every other resource a report mints.
    ///
    /// The count includes the empty parameter set each benchmark is born with, which
    /// makes the ceiling slightly conservative for a project creating benchmarks and
    /// grid points in the same window. That is the safe direction, and it costs a
    /// project nothing that the benchmark ceiling was not already going to cost it.
    #[cfg(feature = "plus")]
    async fn rate_limit(context: &ApiContext, project_id: ProjectId) -> Result<(), HttpError> {
        use crate::error::BencherResource;

        let query_project = QueryProject::get(auth_conn!(context), project_id)?;
        let query_organization = query_project.organization(auth_conn!(context))?;
        let is_claimed = query_organization.is_claimed(auth_conn!(context))?;

        let (start_time, end_time) = context.rate_limiting.window();
        let window_usage: u32 = schema::parameter::table
            .inner_join(schema::benchmark::table)
            .filter(schema::benchmark::project_id.eq(project_id))
            .filter(schema::parameter::created.ge(start_time))
            .filter(schema::parameter::created.le(end_time))
            .count()
            .get_result::<i64>(auth_conn!(context))
            .map_err(crate::error::resource_not_found_err!(
                Parameter,
                (project_id, start_time, end_time)
            ))?
            .try_into()
            .map_err(|e| {
                issue_error(
                    "Failed to count creation",
                    &format!(
                        "Failed to count parameter creation for project ({project_id}) between {start_time} and {end_time}."
                    ),
                    e,
                )
            })?;

        context.rate_limiting.check_claimable_limit(
            is_claimed,
            window_usage,
            |rate_limit| crate::context::RateLimitingError::UnclaimedProject {
                project: query_project.clone(),
                resource: BencherResource::Parameter,
                rate_limit,
            },
            |rate_limit| crate::context::RateLimitingError::ClaimedProject {
                project: query_project.clone(),
                resource: BencherResource::Parameter,
                rate_limit,
            },
        )
    }

    /// The empty parameter set that every benchmark is born with.
    ///
    /// The timestamp is the benchmark's own creation timestamp:
    /// the parameter set is created in the same transaction as its benchmark.
    pub fn empty_set(benchmark_id: BenchmarkId, timestamp: DateTime) -> Self {
        Self::new(benchmark_id, ParameterSet::default(), timestamp)
    }

    /// A parameter set as a report first named it, already canonical.
    pub fn new(benchmark_id: BenchmarkId, set: ParameterSet, timestamp: DateTime) -> Self {
        Self {
            uuid: ParameterUuid::new(),
            benchmark_id,
            set,
            created: timestamp,
            modified: timestamp,
            archived: None,
        }
    }

    pub fn into_query(self, id: ParameterId) -> QueryParameter {
        let Self {
            uuid,
            benchmark_id,
            set,
            created,
            modified,
            archived,
        } = self;
        QueryParameter {
            id,
            uuid,
            benchmark_id,
            set,
            created,
            modified,
            archived,
        }
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = parameter_table)]
pub struct UpdateParameter {
    pub set: Option<ParameterSet>,
    pub modified: DateTime,
    pub archived: Option<Option<DateTime>>,
}

impl From<JsonUpdateParameter> for UpdateParameter {
    fn from(update: JsonUpdateParameter) -> Self {
        let JsonUpdateParameter { archived } = update;
        let modified = DateTime::now();
        let archived = archived.map(|archived| archived.then_some(modified));
        Self {
            set: None,
            modified,
            archived,
        }
    }
}

impl UpdateParameter {
    /// A grid point that reports again is a live grid point.
    fn unarchive() -> Self {
        Self {
            set: None,
            modified: DateTime::now(),
            archived: Some(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, ParameterSet, ParameterUuid};
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
            create_base_entities, create_benchmark, create_branch_with_head, create_parameter,
            create_report, create_report_benchmark, create_testbed, create_version,
            get_empty_parameter, setup_test_db,
        },
    };

    /// Where the blob under test comes from.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Encode {
        /// Written through the `parameter.set` column: the production path.
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

    /// A scalar SQL expression over one stored parameter set.
    fn parameter_text(
        conn: &mut SqliteConnection,
        parameter_id: super::ParameterId,
        sql: &str,
    ) -> String {
        diesel::sql_query(format!("SELECT {sql} AS value FROM parameter WHERE id = ?"))
            .bind::<diesel::sql_types::Integer, _>(parameter_id)
            .get_result::<SqlText>(conn)
            .expect("Failed to read the parameter set")
            .value
    }

    fn parameter_integer(
        conn: &mut SqliteConnection,
        parameter_id: super::ParameterId,
        sql: &str,
    ) -> i32 {
        diesel::sql_query(format!("SELECT {sql} AS value FROM parameter WHERE id = ?"))
            .bind::<diesel::sql_types::Integer, _>(parameter_id)
            .get_result::<SqlInteger>(conn)
            .expect("Failed to read the parameter set")
            .value
    }

    /// A scalar SQL expression over a blob bound directly, for encoded bytes that
    /// never reach the column.
    fn blob_text(conn: &mut SqliteConnection, sql: &str, blob: Vec<u8>) -> String {
        diesel::sql_query(format!("SELECT {sql} AS value"))
            .bind::<diesel::sql_types::Binary, _>(blob)
            .get_result::<SqlText>(conn)
            .expect("Failed to read the encoded parameter set")
            .value
    }

    fn blob_integer(conn: &mut SqliteConnection, sql: &str, blob: Vec<u8>) -> i32 {
        diesel::sql_query(format!("SELECT {sql} AS value"))
            .bind::<diesel::sql_types::Binary, _>(blob)
            .get_result::<SqlInteger>(conn)
            .expect("Failed to read the encoded parameter set")
            .value
    }

    /// Mint a parameter set with `SQLite`'s `jsonb()`, the migration's write path.
    fn mint_parameter(
        conn: &mut SqliteConnection,
        benchmark_id: BenchmarkId,
        canonical: &str,
    ) -> QueryResult<usize> {
        diesel::sql_query(
            r#"INSERT INTO parameter(uuid, benchmark_id, "set", created, modified)
               VALUES (?, ?, jsonb(?), 0, 0)"#,
        )
        .bind::<diesel::sql_types::Text, _>(ParameterUuid::new().to_string())
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

    /// The parameter set a benchmark was born with, or a freshly written one.
    fn write_parameter(
        conn: &mut SqliteConnection,
        benchmark_id: BenchmarkId,
        parameters: &ParameterSet,
    ) -> super::ParameterId {
        if parameters.is_empty() {
            get_empty_parameter(conn, benchmark_id)
        } else {
            create_parameter(conn, benchmark_id, parameters)
        }
    }

    // The encoder has to be byte identical to SQLite's `jsonb()` over the same
    // canonical text, because `UNIQUE(benchmark_id, "set")` compares bytes and
    // both writers reach that column: the migration mints the empty set with
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

                    let parameter_id = write_parameter(&mut conn, benchmark_id, &parameters);
                    assert_eq!(
                        parameter_text(&mut conn, parameter_id, "hex(\"set\")"),
                        minted,
                        "{name}: the written bytes must be the bytes jsonb() mints"
                    );
                    assert_eq!(
                        parameter_integer(&mut conn, parameter_id, "json_valid(\"set\", 8)"),
                        1,
                        "{name}: SQLite's JSON functions must accept the written bytes"
                    );
                    assert_eq!(
                        parameter_text(&mut conn, parameter_id, "json(\"set\")"),
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

    // A parameter set read back out of the column has to be the set that was
    // written, whichever writer wrote it, and re-canonicalizing it has to land on
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
        // SQLite mints every set under this benchmark, the empty one included, so
        // the Diesel written set it was born with is cleared out of the way first.
        diesel::delete(schema::parameter::table.filter(schema::parameter::benchmark_id.eq(minted)))
            .execute(&mut conn)
            .expect("Failed to clear the minted benchmark's parameter sets");

        for (name, canonical, encode) in CONFORMANCE {
            if *encode != Encode::Column {
                continue;
            }
            let parameters = parameters(canonical);

            let parameter_id = write_parameter(&mut conn, written, &parameters);
            let read: ParameterSet = schema::parameter::table
                .filter(schema::parameter::id.eq(parameter_id))
                .select(schema::parameter::set)
                .first(&mut conn)
                .expect("Failed to read back a written parameter set");
            assert_eq!(read, parameters, "{name}: written and read back");
            assert_eq!(
                read.canonical(),
                *canonical,
                "{name}: written stays canonical"
            );

            mint_parameter(&mut conn, minted, canonical).expect("Failed to mint a parameter set");
            let read: ParameterSet = schema::parameter::table
                .filter(schema::parameter::benchmark_id.eq(minted))
                .order(schema::parameter::id.desc())
                .select(schema::parameter::set)
                .first(&mut conn)
                .expect("Failed to read back a minted parameter set");
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

            // The empty set is already there, written through Diesel when the
            // benchmark was born, so for that one set the mint is what collides.
            let minted = mint_parameter(&mut conn, benchmark_id, canonical);
            let written = insert_parameter(&mut conn, benchmark_id, &parameters(canonical));

            assert!(
                is_unique_violation(&minted) || is_unique_violation(&written),
                r#"{name}: the two writers must collide on UNIQUE(benchmark_id, "set")"#
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
                count_parameters(&mut conn, benchmark_id),
                expected,
                "{name}: one row per distinct parameter set"
            );
        }
    }

    fn insert_parameter(
        conn: &mut SqliteConnection,
        benchmark_id: BenchmarkId,
        parameters: &ParameterSet,
    ) -> QueryResult<usize> {
        diesel::insert_into(schema::parameter::table)
            .values((
                schema::parameter::uuid.eq(ParameterUuid::new()),
                schema::parameter::benchmark_id.eq(benchmark_id),
                schema::parameter::set.eq(parameters),
                schema::parameter::created.eq(DateTime::TEST),
                schema::parameter::modified.eq(DateTime::TEST),
            ))
            .execute(conn)
    }

    fn count_parameters(conn: &mut SqliteConnection, benchmark_id: BenchmarkId) -> i64 {
        schema::parameter::table
            .filter(schema::parameter::benchmark_id.eq(benchmark_id))
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

        insert_parameter(&mut conn, benchmark_id, &parameters(r#"{"b": 1, "a": 2}"#))
            .expect("Failed to insert parameter");
        let collision =
            insert_parameter(&mut conn, benchmark_id, &parameters(r#"{"a": 2, "b": 1}"#));

        assert!(
            collision.is_err(),
            r#"logically equal parameter sets must collide on UNIQUE(benchmark_id, "set")"#
        );
        // The empty set the benchmark was born with, plus the one that landed.
        assert_eq!(count_parameters(&mut conn, benchmark_id), 2);
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

        insert_parameter(&mut conn, benchmark_id, &parameters(r#"{"n": 16}"#))
            .expect("Failed to insert parameter");
        for spelling in [r#"{"n": 16.0}"#, r#"{"n": 1.6e1}"#] {
            assert!(
                insert_parameter(&mut conn, benchmark_id, &parameters(spelling)).is_err(),
                "{spelling} must collide with 16"
            );
        }

        assert_eq!(count_parameters(&mut conn, benchmark_id), 2);
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

        let grid_point = parameters(r#"{"size_mb": 16}"#);
        insert_parameter(&mut conn, first, &grid_point).expect("Failed to insert parameter");
        insert_parameter(&mut conn, second, &grid_point).expect("Failed to insert parameter");

        assert_eq!(count_parameters(&mut conn, first), 2);
        assert_eq!(count_parameters(&mut conn, second), 2);
    }

    /// Seed the pre-migration shape: benchmarks and `report_benchmark` rows with
    /// no `parameter_id`, written as raw SQL because the Diesel DSL describes the
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
    fn migration_backfills_empty_parameter_sets() {
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
            let backfilled: Vec<ParameterSet> = schema::parameter::table
                .filter(schema::parameter::benchmark_id.eq(benchmark_id))
                .select(schema::parameter::set)
                .load(&mut conn)
                .expect("Failed to load parameters");
            assert_eq!(
                backfilled,
                vec![ParameterSet::default()],
                "every benchmark gets exactly one empty parameter set"
            );

            // The migration mints the canonical empty object in SQL, so a set minted
            // in Rust has to be byte identical to it.
            assert!(
                insert_parameter(&mut conn, benchmark_id, &ParameterSet::default()).is_err(),
                "the backfilled empty set must collide with a Rust minted one"
            );
        }

        let report_benchmarks: Vec<(BenchmarkId, super::ParameterId)> =
            schema::report_benchmark::table
                .select((
                    schema::report_benchmark::benchmark_id,
                    schema::report_benchmark::parameter_id,
                ))
                .load(&mut conn)
                .expect("Failed to load report benchmarks");
        assert_eq!(report_benchmarks.len(), 3);
        for (benchmark_id, parameter_id) in report_benchmarks {
            let empty_set_id = super::QueryParameter::get_empty_set_id(&mut conn, benchmark_id)
                .expect("Failed to get the empty parameter set");
            assert_eq!(
                parameter_id, empty_set_id,
                "every report benchmark points at its own benchmark's empty set"
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
    /// on the unique key over the report, iteration, benchmark, and parameter set.
    fn duplicate_report_benchmark_key(conn: &mut SqliteConnection) -> QueryResult<usize> {
        diesel::sql_query(
            "INSERT INTO report_benchmark (uuid, report_id, iteration, benchmark_id, parameter_id)
                SELECT '00000000-0000-0000-0000-000000000013',
                    report_id, iteration, benchmark_id, parameter_id
                FROM report_benchmark
                WHERE id = 1",
        )
        .execute(conn)
    }

    /// Re-insert an existing `report_benchmark` row under a new iteration, which
    /// collides on the unique uuid.
    fn duplicate_report_benchmark_uuid(conn: &mut SqliteConnection) -> QueryResult<usize> {
        diesel::sql_query(
            "INSERT INTO report_benchmark (uuid, report_id, iteration, benchmark_id, parameter_id)
                SELECT uuid, report_id, 42, benchmark_id, parameter_id
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
                "index_report_benchmark_parameter".to_owned(),
                "index_report_benchmark_report_iteration_benchmark_parameter".to_owned(),
                "index_report_benchmark_uuid".to_owned(),
            ],
            "the rebuilt table's indexes are the named ones the migration builds"
        );

        let repeated_key = duplicate_report_benchmark_key(&mut conn);
        assert!(
            is_unique_violation(&repeated_key),
            "a repeated report, iteration, benchmark, and parameter set collides"
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
        assert_eq!(count_parameters(&mut conn, benchmark_id), 1);

        conn.batch_execute("PRAGMA foreign_keys = OFF")
            .expect("Failed to disable foreign keys");
        revert_to_parameter_migration(&mut conn);
        conn.run_pending_migrations(crate::MIGRATIONS)
            .expect("Failed to re-apply the parameter migration");
        conn.batch_execute("PRAGMA foreign_keys = ON")
            .expect("Failed to enable foreign keys");

        assert_eq!(count_parameters(&mut conn, benchmark_id), 1);
    }
}
