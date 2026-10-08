#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::tests_outside_test_module,
    clippy::too_many_lines,
    reason = "integration test file"
)]
//! Integration tests for the console's dimension lists.

use bencher_api_tests::{
    TestServer, TestUser,
    helpers::{base_timestamp, get_project_id, grant_project_role},
};
use bencher_json::{
    BenchmarkUuid, BranchUuid, DateTime, HeadUuid, JsonProjectKeyCreated, MeasureUuid, MetricName,
    MetricUuid, ParameterSet, ReportBenchmarkUuid, ReportUuid, TestbedUuid, ThresholdUuid,
    VariantUuid, VersionUuid, project::Visibility,
};
use bencher_rbac::project::Role;
use bencher_schema::{context::DbConnection, macros::sql::last_insert_rowid, schema};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;
use serde_json::{Value, json};

// GET /v0/projects/{project}/console/branches - the active branches by name, each with when it
// was created, the branch its current head started from, the hash and time of its newest report
// on any of its heads, and its thresholds split into those that move with it and those another
// archived dimension holds; the archived branch says when it was archived
// Kills: a start point read from a replaced head, or from a head matched by the wrong ID.
#[tokio::test]
async fn console_branches_list_each_branch_with_its_last_report_and_thresholds() {
    let server = TestServer::new().await;
    let f = seed_project(&server, "dmbranches").await;
    seed_project(&server, "dmbranchesother").await;

    let (status, body) = get(&server, &f.user.token, &f.slug, "branches", "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "total": 4,
            "active": 4,
            "archived": 1,
            "branches": [
                {
                    "uuid": f.devel,
                    "name": "devel",
                    "slug": "dev",
                    "hash": V1_HASH,
                    "created": millis(5),
                    "last_report": millis(100),
                    "thresholds": 0,
                    "held_thresholds": 0,
                },
                {
                    "uuid": f.feature,
                    "name": "feature",
                    "slug": "feature",
                    "start_point": "devel",
                    "hash": V2_HASH,
                    "created": millis(10),
                    "last_report": millis(400),
                    "thresholds": 1,
                    "held_thresholds": 0,
                },
                {
                    "uuid": f.main,
                    "name": "main",
                    "slug": "main",
                    "hash": V1_HASH,
                    "created": millis(0),
                    "last_report": millis(300),
                    "thresholds": 1,
                    "held_thresholds": 2,
                },
                {
                    "uuid": f.old,
                    "name": "old",
                    "slug": "stale",
                    "created": millis(20),
                    "thresholds": 0,
                    "held_thresholds": 0,
                },
            ],
        })
    );

    let (status, body) = get(&server, &f.user.token, &f.slug, "branches", "archived=true").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "total": 1,
            "active": 4,
            "archived": 1,
            "branches": [{
                "uuid": f.gone,
                "name": "gone",
                "slug": "gone",
                "hash": V1_HASH,
                "created": millis(6),
                "archived": millis(30),
                "last_report": millis(160),
                "thresholds": 1,
                "held_thresholds": 1,
            }],
        })
    );
}

// GET /v0/projects/{project}/console/testbeds - each testbed with its spec, the time of the report
// that ended last on it, and its thresholds split by whether their branch and measure are active
// Kills: a testbed's newest report taken by the order the API took them, so a backfilled run
// counts as its newest.
#[tokio::test]
async fn console_testbeds_list_each_testbed_with_its_spec_and_thresholds() {
    let server = TestServer::new().await;
    let f = seed_project(&server, "dmtestbeds").await;
    seed_project(&server, "dmtestbedsother").await;

    let (status, body) = get(&server, &f.user.token, &f.slug, "testbeds", "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "total": 2,
            "active": 2,
            "archived": 1,
            "testbeds": [
                {
                    "uuid": f.arm,
                    "name": "arm",
                    "slug": "arm",
                    "spec": "arm-8",
                    "created": millis(1),
                    "last_report": millis(300),
                    "thresholds": 1,
                    "held_thresholds": 0,
                },
                {
                    "uuid": f.linux,
                    "name": "linux",
                    "slug": "linux",
                    "created": millis(0),
                    "last_report": millis(150),
                    "thresholds": 1,
                    "held_thresholds": 2,
                },
            ],
        })
    );

    let (status, body) = get(&server, &f.user.token, &f.slug, "testbeds", "archived=true").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["testbeds"],
        json!([{
            "uuid": f.macos,
            "name": "macos",
            "slug": "macos",
            "created": millis(2),
            "archived": millis(40),
            "thresholds": 1,
            "held_thresholds": 1,
        }])
    );
}

// GET /v0/projects/{project}/console/benchmarks - each benchmark with its active variants that have
// a report and the time of its newest report
// Kills: counting a variant that never reported, or an archived one that did.
#[tokio::test]
async fn console_benchmarks_list_each_benchmark_with_its_variants() {
    let server = TestServer::new().await;
    let f = seed_project(&server, "dmbenchmarks").await;
    seed_project(&server, "dmbenchmarksother").await;

    let (status, body) = get(&server, &f.user.token, &f.slug, "benchmarks", "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "total": 2,
            "active": 2,
            "archived": 1,
            "benchmarks": [
                {
                    "uuid": f.alpha,
                    "name": "alpha",
                    "slug": "alpha",
                    "variants": 1,
                    "created": millis(0),
                    "last_report": millis(300),
                },
                {
                    "uuid": f.beta,
                    "name": "beta",
                    "slug": "beta",
                    "variants": 1,
                    "created": millis(1),
                    "last_report": millis(400),
                },
            ],
        })
    );

    let (status, body) = get(
        &server,
        &f.user.token,
        &f.slug,
        "benchmarks",
        "archived=true",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["benchmarks"],
        json!([{
            "uuid": f.idle,
            "name": "idle",
            "slug": "idle",
            "variants": 0,
            "created": millis(2),
            "archived": millis(50),
        }])
    );
}

// GET /v0/projects/{project}/console/benchmarks - a BMF v1 benchmark that reports only
// parameterized variants counts those, not the empty variant it was created with
// Kills: counting every active variant of the benchmark, reported or not.
#[tokio::test]
async fn console_benchmarks_count_the_variants_a_report_carries() {
    let server = TestServer::new().await;
    let user = server
        .signup("Dimension User", "dmvariants@example.com")
        .await;
    let org = server.create_org(&user, "dmvariants org").await;
    let project = server
        .create_project(&user, &org, "dmvariants project")
        .await;
    let slug = project.slug.to_string();
    let results = json!({
        "alpha": [
            { "parameters": { "size": 2 }, "measures": { "latency": { "value": 1.0 } } },
            { "parameters": { "size": 10 }, "measures": { "latency": { "value": 2.0 } } },
        ],
        "beta": [{ "measures": { "latency": { "value": 3.0 } } }],
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{slug}/reports")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&json!({
            "branch": "main",
            "testbed": "localhost",
            "start_time": "2024-01-01T00:00:00Z",
            "end_time": "2024-01-01T00:01:00Z",
            "results": [results.to_string()],
            "bmf_version": 1,
            "settings": { "adapter": "json" },
        }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED, "report");

    let (status, body) = get(&server, &user.token, &slug, "benchmarks", "").await;
    assert_eq!(status, StatusCode::OK);
    let variants = body["benchmarks"]
        .as_array()
        .expect("Failed to read the rows")
        .iter()
        .map(|row| (row["name"].as_str(), row["variants"].as_u64()))
        .collect::<Vec<_>>();
    assert_eq!(
        variants,
        [(Some("alpha"), Some(2)), (Some("beta"), Some(1))]
    );
}

// GET /v0/projects/{project}/console/measures - each measure with its units, the time of the
// newest report with a value for it, and its thresholds split by whether their branch and testbed
// are active
#[tokio::test]
async fn console_measures_list_each_measure_with_its_units_and_thresholds() {
    let server = TestServer::new().await;
    let f = seed_project(&server, "dmmeasures").await;
    seed_project(&server, "dmmeasuresother").await;

    let (status, body) = get(&server, &f.user.token, &f.slug, "measures", "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "total": 2,
            "active": 2,
            "archived": 1,
            "measures": [
                {
                    "uuid": f.cycles,
                    "name": "cycles",
                    "slug": "cycles",
                    "units": "cycles",
                    "created": millis(1),
                    "last_report": millis(300),
                    "thresholds": 1,
                    "held_thresholds": 1,
                },
                {
                    "uuid": f.latency,
                    "name": "latency",
                    "slug": "latency",
                    "units": "ns",
                    "created": millis(0),
                    "last_report": millis(400),
                    "thresholds": 1,
                    "held_thresholds": 2,
                },
            ],
        })
    );

    let (status, body) = get(&server, &f.user.token, &f.slug, "measures", "archived=true").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["measures"],
        json!([{
            "uuid": f.retired,
            "name": "retired",
            "slug": "retired",
            "units": "x",
            "created": millis(2),
            "archived": millis(50),
            "thresholds": 1,
            "held_thresholds": 0,
        }])
    );
}

// GET /v0/projects/{project}/console/branches - each sort in each direction, a time sort newest
// first when no direction is given, ties broken by name in the same direction, and the never
// reported last either way
#[tokio::test]
async fn console_branches_sort_by_name_created_and_last_used() {
    let server = TestServer::new().await;
    let f = seed_project(&server, "dmsort").await;
    // A second branch created with main sorts after it by name, either way.
    insert_branch(
        &mut server.db_conn(),
        &f.base,
        "main2",
        &BranchSpec {
            created: 0,
            ..BranchSpec::default()
        },
    );

    for (query, expected) in [
        ("", &["devel", "feature", "main", "main2", "old"][..]),
        (
            "sort=name&direction=asc",
            &["devel", "feature", "main", "main2", "old"],
        ),
        (
            "sort=name&direction=desc",
            &["old", "main2", "main", "feature", "devel"],
        ),
        (
            "sort=created",
            &["old", "feature", "devel", "main2", "main"],
        ),
        (
            "sort=created&direction=desc",
            &["old", "feature", "devel", "main2", "main"],
        ),
        (
            "sort=created&direction=asc",
            &["main", "main2", "devel", "feature", "old"],
        ),
        (
            "sort=last_used",
            &["feature", "main", "devel", "old", "main2"],
        ),
        (
            "sort=last_used&direction=desc",
            &["feature", "main", "devel", "old", "main2"],
        ),
        (
            "sort=last_used&direction=asc",
            &["devel", "main", "feature", "main2", "old"],
        ),
    ] {
        let (status, body) = get(&server, &f.user.token, &f.slug, "branches", query).await;
        assert_eq!(status, StatusCode::OK, "{query}");
        assert_eq!(names(&body, "branches"), expected, "{query}");
    }
}

// GET /v0/projects/{project}/console/{testbeds,benchmarks,measures} - newest report first finds
// each dimension's own newest report
#[tokio::test]
async fn console_dimensions_sort_by_last_used() {
    let server = TestServer::new().await;
    let f = seed_project(&server, "dmlastused").await;

    for (dimension, expected) in [
        ("testbeds", ["arm", "linux"]),
        ("benchmarks", ["beta", "alpha"]),
        ("measures", ["latency", "cycles"]),
    ] {
        let (status, body) =
            get(&server, &f.user.token, &f.slug, dimension, "sort=last_used").await;
        assert_eq!(status, StatusCode::OK, "{dimension}");
        assert_eq!(names(&body, dimension), expected, "{dimension}");
        let (_, body) = get(
            &server,
            &f.user.token,
            &f.slug,
            dimension,
            "sort=last_used&direction=asc",
        )
        .await;
        assert_eq!(
            names(&body, dimension),
            [expected[1], expected[0]],
            "{dimension}"
        );
    }
}

// GET /v0/projects/{project}/console/branches - a page past the first, and a search by name, slug,
// or UUID that narrows the rows and the total but not the active and archived counts
#[tokio::test]
async fn console_branches_page_and_search() {
    let server = TestServer::new().await;
    let f = seed_project(&server, "dmpage").await;

    for (query, expected, total) in [
        (
            "sort=created&direction=asc&per_page=2",
            &["main", "devel"][..],
            4,
        ),
        (
            "sort=created&direction=asc&per_page=2&page=2",
            &["feature", "old"],
            4,
        ),
        ("sort=created&direction=asc&per_page=2&page=3", &[], 4),
        ("search=eat", &["feature"], 1),
        ("search=vel", &["devel"], 1),
        ("search=tal", &["old"], 1),
        ("search=one&archived=true", &["gone"], 1),
        (&format!("search={}", f.old), &["old"], 1),
        ("search=nothing", &[], 0),
    ] {
        let (status, body) = get(&server, &f.user.token, &f.slug, "branches", query).await;
        assert_eq!(status, StatusCode::OK, "{query}");
        assert_eq!(names(&body, "branches"), expected, "{query}");
        assert_eq!(
            (&body["total"], &body["active"], &body["archived"]),
            (&json!(total), &json!(4), &json!(1)),
            "{query}"
        );
    }
}

// PATCH /v0/projects/{project}/{branches,testbeds,measures}/{dimension} - archiving a dimension
// archives every threshold on it, and unarchiving it brings back each threshold on it whose other
// dimensions are active and leaves the rest archived
#[tokio::test]
async fn archiving_a_dimension_archives_its_thresholds() {
    let server = TestServer::new().await;
    let f = seed_project(&server, "dmarchive").await;
    let all = [f.t_main, f.t_feature];

    assert_eq!(listed_thresholds(&server, &f, false).await, all);

    patch_archived(&server, &f, "branches/main", true).await;
    assert_eq!(listed_thresholds(&server, &f, false).await, [f.t_feature]);
    assert!(
        listed_thresholds(&server, &f, true)
            .await
            .contains(&f.t_main)
    );
    assert_eq!(
        counts(&server, &f, "branches", "archived=true", "main").await,
        (1, 2)
    );
    assert_eq!(counts(&server, &f, "testbeds", "", "linux").await, (0, 3));

    patch_archived(&server, &f, "testbeds/linux", true).await;
    patch_archived(&server, &f, "branches/main", false).await;
    assert_eq!(listed_thresholds(&server, &f, false).await, [f.t_feature]);
    assert_eq!(counts(&server, &f, "branches", "", "main").await, (0, 3));
    assert_eq!(
        counts(&server, &f, "testbeds", "archived=true", "linux").await,
        (1, 2)
    );

    patch_archived(&server, &f, "testbeds/linux", false).await;
    assert_eq!(listed_thresholds(&server, &f, false).await, all);

    patch_archived(&server, &f, "measures/latency", true).await;
    assert_eq!(listed_thresholds(&server, &f, false).await, [f.t_feature]);
    patch_archived(&server, &f, "measures/latency", false).await;
    assert_eq!(listed_thresholds(&server, &f, false).await, all);
}

// GET /v0/projects/{project}/console/{dimension} - a signed-in reader or the project's own key may
// read a public project; anyone else needs to be signed in and a member of a private one
#[tokio::test]
async fn console_dimensions_take_a_signed_in_reader() {
    use StatusCode as S;

    let server = TestServer::new().await;
    let public = seed_project(&server, "dmauth").await;
    let private = seed_project(&server, "dmauthprivate").await;
    diesel::update(schema::project::table.filter(schema::project::slug.eq(&private.slug)))
        .set(schema::project::visibility.eq(Visibility::Private))
        .execute(&mut server.db_conn())
        .expect("Failed to make the project private");
    let other = seed_project(&server, "dmauthother").await;
    let viewer = server
        .signup("Dimension Viewer", "dmauthviewer@example.com")
        .await;
    grant_project_role(&server, &viewer, &public.slug, Role::Viewer);
    grant_project_role(&server, &viewer, &private.slug, Role::Viewer);
    let stranger = server
        .signup("Dimension Stranger", "dmauthstranger@example.com")
        .await;
    let public_key = create_key(&server, &public.user, &public.slug).await;
    let private_key = create_key(&server, &private.user, &private.slug).await;
    let other_key = create_key(&server, &other.user, &other.slug).await;

    for (f, own_key, anonymous, strangers, others_key) in [
        (&public, &public_key, S::UNAUTHORIZED, S::OK, S::FORBIDDEN),
        (
            &private,
            &private_key,
            S::NOT_FOUND,
            S::NOT_FOUND,
            S::NOT_FOUND,
        ),
    ] {
        for (token, expected) in [
            (None, anonymous),
            (Some(viewer.token.as_str()), S::OK),
            (Some(stranger.token.as_str()), strangers),
            (Some(own_key.as_str()), S::OK),
            (Some(other_key.as_str()), others_key),
        ] {
            for dimension in ["branches", "testbeds", "benchmarks", "measures"] {
                let (status, _) = request(
                    &server,
                    token,
                    &format!("/v0/projects/{}/console/{dimension}", f.slug),
                )
                .await;
                assert_eq!(status, expected, "{} {dimension} {token:?}", f.slug);
            }
        }
    }
}

const V1_HASH: &str = "1111111111111111111111111111111111111111";
const V2_HASH: &str = "2222222222222222222222222222222222222222";

struct Fixture {
    user: TestUser,
    slug: String,
    base: SeededBase,
    main: BranchUuid,
    feature: BranchUuid,
    devel: BranchUuid,
    old: BranchUuid,
    gone: BranchUuid,
    linux: TestbedUuid,
    arm: TestbedUuid,
    macos: TestbedUuid,
    alpha: BenchmarkUuid,
    beta: BenchmarkUuid,
    idle: BenchmarkUuid,
    latency: MeasureUuid,
    cycles: MeasureUuid,
    retired: MeasureUuid,
    /// The threshold on main, linux, and latency.
    t_main: ThresholdUuid,
    /// The threshold on feature, arm, and cycles.
    t_feature: ThresholdUuid,
}

/// Active branches main, devel (slug dev, its report on the head it started from main, replaced
/// by one started from nothing), feature (from devel), and old (slug stale, never reported), and
/// gone (archived, reported on two heads); active testbeds linux and arm (with a spec), and macos (archived); active
/// benchmarks alpha (one of its two variants archived) and beta, and idle (archived, neither of
/// its variants reported); active measures latency and cycles, and retired (archived). Reports, in
/// the order the API took them: main 50 (with alpha's archived variant too), devel 100, gone 150,
/// gone 160 (on the head that replaced the first), main 300 (ending when gone 160 did), feature 400
/// (ending before gone 150). Thresholds: main on linux and
/// latency, main on macos, main on retired, gone on linux and latency, feature on arm and cycles,
/// gone on macos and cycles.
async fn seed_project(server: &TestServer, label: &str) -> Fixture {
    let user = server
        .signup("Dimension User", &format!("{label}@example.com"))
        .await;
    let org = server.create_org(&user, &format!("{label} org")).await;
    let project = server
        .create_project(&user, &org, &format!("{label} project"))
        .await;
    let slug = project.slug.to_string();
    let project_id = get_project_id(server, &slug);

    let mut conn = server.db_conn();
    let conn = &mut conn;
    let base = insert_base(conn, project_id);

    let main = insert_branch(conn, &base, "main", &BranchSpec::default());
    let devel = insert_branch(
        conn,
        &base,
        "devel",
        &BranchSpec {
            slug: Some("dev"),
            created: 5,
            start_point: Some(main.head_version),
            ..BranchSpec::default()
        },
    );
    // Replaced before the next branch, so no later branch shares its ID with its head.
    let (_, devel_head_version) = reset_head(conn, &devel);
    let gone = insert_branch(
        conn,
        &base,
        "gone",
        &BranchSpec {
            created: 6,
            archived: Some(30),
            ..BranchSpec::default()
        },
    );
    let feature = insert_branch(
        conn,
        &base,
        "feature",
        &BranchSpec {
            created: 10,
            start_point: Some(devel_head_version),
            ..BranchSpec::default()
        },
    );
    let old = insert_branch(
        conn,
        &base,
        "old",
        &BranchSpec {
            slug: Some("stale"),
            created: 20,
            ..BranchSpec::default()
        },
    );

    let spec = insert_spec(conn, "arm-8");
    let linux = insert_testbed(conn, project_id, "linux", 0, None, None);
    let arm = insert_testbed(conn, project_id, "arm", 1, None, Some(spec));
    let macos = insert_testbed(conn, project_id, "macos", 2, Some(40), None);

    let (alpha, alpha_variant) = insert_benchmark(conn, project_id, "alpha", 0, None);
    let alpha_archived = insert_variant(conn, alpha.0, 1, Some(60));
    let (beta, beta_variant) = insert_benchmark(conn, project_id, "beta", 1, None);
    let (idle, _) = insert_benchmark(conn, project_id, "idle", 2, Some(50));
    insert_variant(conn, idle.0, 2, None);

    let latency = insert_measure(conn, project_id, "latency", "ns", 0, None);
    let cycles = insert_measure(conn, project_id, "cycles", "cycles", 1, None);
    let retired = insert_measure(conn, project_id, "retired", "x", 2, Some(50));

    let v1 = base.version;
    let v2 = insert_version(conn, project_id, 2, V2_HASH);
    let first = insert_report(
        conn,
        project_id,
        (main.head, linux.0, v2),
        50,
        (alpha_variant, &[latency.0]),
    );
    insert_report_benchmark(conn, first, alpha_archived);
    insert_report(
        conn,
        project_id,
        (devel.head, linux.0, v1),
        100,
        (alpha_variant, &[latency.0]),
    );
    insert_report(
        conn,
        project_id,
        (gone.head, linux.0, v1),
        150,
        (beta_variant, &[latency.0]),
    );
    let (gone_head, _) = reset_head(conn, &gone);
    insert_report(
        conn,
        project_id,
        (gone_head, arm.0, v1),
        160,
        (beta_variant, &[]),
    );
    let tied = insert_report(
        conn,
        project_id,
        (main.head, arm.0, v1),
        300,
        (alpha_variant, &[latency.0, cycles.0]),
    );
    end_report_at(conn, tied, 159);
    let backfilled = insert_report(
        conn,
        project_id,
        (feature.head, linux.0, v2),
        400,
        (beta_variant, &[latency.0]),
    );
    end_report_at(conn, backfilled, 120);

    let t_main = insert_threshold(conn, project_id, (main.id, linux.0, latency.0));
    insert_threshold(conn, project_id, (main.id, macos.0, latency.0));
    insert_threshold(conn, project_id, (main.id, linux.0, retired.0));
    insert_threshold(conn, project_id, (gone.id, linux.0, latency.0));
    let t_feature = insert_threshold(conn, project_id, (feature.id, arm.0, cycles.0));
    insert_threshold(conn, project_id, (gone.id, macos.0, cycles.0));

    Fixture {
        user,
        slug,
        base,
        main: main.uuid,
        feature: feature.uuid,
        devel: devel.uuid,
        old: old.uuid,
        gone: gone.uuid,
        linux: linux.1,
        arm: arm.1,
        macos: macos.1,
        alpha: alpha.1,
        beta: beta.1,
        idle: idle.1,
        latency: latency.1,
        cycles: cycles.1,
        retired: retired.1,
        t_main,
        t_feature,
    }
}

struct SeededBase {
    project: i32,
    version: i32,
}

fn insert_base(conn: &mut DbConnection, project_id: i32) -> SeededBase {
    SeededBase {
        project: project_id,
        version: insert_version(conn, project_id, 1, V1_HASH),
    }
}

fn insert_version(conn: &mut DbConnection, project_id: i32, number: i32, hash: &str) -> i32 {
    diesel::insert_into(schema::version::table)
        .values((
            schema::version::uuid.eq(VersionUuid::new()),
            schema::version::project_id.eq(project_id),
            schema::version::number.eq(number),
            schema::version::hash.eq(hash),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a version");
    last_id(conn)
}

struct SeededBranch {
    id: i32,
    uuid: BranchUuid,
    head: i32,
    head_version: i32,
}

#[derive(Default)]
struct BranchSpec {
    /// The name when not given.
    slug: Option<&'static str>,
    created: i64,
    start_point: Option<i32>,
    archived: Option<i64>,
}

fn insert_branch(
    conn: &mut DbConnection,
    base: &SeededBase,
    name: &str,
    spec: &BranchSpec,
) -> SeededBranch {
    let uuid = BranchUuid::new();
    diesel::insert_into(schema::branch::table)
        .values((
            schema::branch::uuid.eq(uuid),
            schema::branch::project_id.eq(base.project),
            schema::branch::name.eq(name),
            schema::branch::slug.eq(spec.slug.unwrap_or(name)),
            schema::branch::created.eq(seconds(spec.created)),
            schema::branch::modified.eq(seconds(spec.created)),
            schema::branch::archived.eq(spec.archived.map(seconds)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a branch");
    let id = last_id(conn);
    let (head, head_version) = insert_head(conn, id, spec.start_point, base.version);
    SeededBranch {
        id,
        uuid,
        head,
        head_version,
    }
}

/// A head for the branch, made its current one, holding one version.
fn insert_head(
    conn: &mut DbConnection,
    branch_id: i32,
    start_point: Option<i32>,
    version: i32,
) -> (i32, i32) {
    diesel::insert_into(schema::head::table)
        .values((
            schema::head::uuid.eq(HeadUuid::new()),
            schema::head::branch_id.eq(branch_id),
            schema::head::start_point_id.eq(start_point),
            schema::head::created.eq(seconds(0)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a head");
    let head = last_id(conn);
    diesel::update(schema::branch::table.filter(schema::branch::id.eq(branch_id)))
        .set(schema::branch::head_id.eq(head))
        .execute(&mut *conn)
        .expect("Failed to point the branch at its head");
    diesel::insert_into(schema::head_version::table)
        .values((
            schema::head_version::head_id.eq(head),
            schema::head_version::version_id.eq(version),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a head version");
    (head, last_id(conn))
}

/// Replace the branch's current head with a new one that starts from nothing.
fn reset_head(conn: &mut DbConnection, branch: &SeededBranch) -> (i32, i32) {
    diesel::update(schema::head::table.filter(schema::head::id.eq(branch.head)))
        .set(schema::head::replaced.eq(seconds(200)))
        .execute(&mut *conn)
        .expect("Failed to replace the head");
    let version = schema::head_version::table
        .filter(schema::head_version::id.eq(branch.head_version))
        .select(schema::head_version::version_id)
        .first::<i32>(&mut *conn)
        .expect("Failed to read the head's version");
    insert_head(conn, branch.id, None, version)
}

fn insert_spec(conn: &mut DbConnection, name: &str) -> i32 {
    diesel::insert_into(schema::spec::table)
        .values((
            schema::spec::uuid.eq(uuid::Uuid::new_v4().to_string()),
            schema::spec::name.eq(name),
            schema::spec::slug.eq(format!("{name}-{}", uuid::Uuid::new_v4())),
            schema::spec::os.eq("linux"),
            schema::spec::architecture.eq("aarch64"),
            schema::spec::cpu.eq(8),
            schema::spec::memory.eq(0x8000_0000i64),
            schema::spec::disk.eq(0x8000_0000i64),
            schema::spec::network.eq(false),
            schema::spec::created.eq(seconds(0)),
            schema::spec::modified.eq(seconds(0)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a spec");
    last_id(conn)
}

fn insert_testbed(
    conn: &mut DbConnection,
    project_id: i32,
    name: &str,
    created: i64,
    archived: Option<i64>,
    spec: Option<i32>,
) -> (i32, TestbedUuid) {
    let uuid = TestbedUuid::new();
    diesel::insert_into(schema::testbed::table)
        .values((
            schema::testbed::uuid.eq(uuid),
            schema::testbed::project_id.eq(project_id),
            schema::testbed::name.eq(name),
            schema::testbed::slug.eq(name),
            schema::testbed::spec_id.eq(spec),
            schema::testbed::created.eq(seconds(created)),
            schema::testbed::modified.eq(seconds(created)),
            schema::testbed::archived.eq(archived.map(seconds)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a testbed");
    (last_id(conn), uuid)
}

/// A benchmark and its empty variant.
fn insert_benchmark(
    conn: &mut DbConnection,
    project_id: i32,
    name: &str,
    created: i64,
    archived: Option<i64>,
) -> ((i32, BenchmarkUuid), i32) {
    let uuid = BenchmarkUuid::new();
    diesel::insert_into(schema::benchmark::table)
        .values((
            schema::benchmark::uuid.eq(uuid),
            schema::benchmark::project_id.eq(project_id),
            schema::benchmark::name.eq(name),
            schema::benchmark::slug.eq(name),
            schema::benchmark::created.eq(seconds(created)),
            schema::benchmark::modified.eq(seconds(created)),
            schema::benchmark::archived.eq(archived.map(seconds)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a benchmark");
    let id = last_id(conn);
    let variant = insert_variant(conn, id, 0, None);
    ((id, uuid), variant)
}

/// A variant whose one parameter is `n`; zero makes the empty variant.
fn insert_variant(conn: &mut DbConnection, benchmark_id: i32, n: u8, archived: Option<i64>) -> i32 {
    let parameters = if n == 0 {
        ParameterSet::default()
    } else {
        serde_json::from_value(json!({ "n": n })).expect("Failed to build a parameter set")
    };
    diesel::insert_into(schema::variant::table)
        .values((
            schema::variant::uuid.eq(VariantUuid::new()),
            schema::variant::benchmark_id.eq(benchmark_id),
            schema::variant::parameters.eq(parameters),
            schema::variant::created.eq(base_timestamp()),
            schema::variant::modified.eq(base_timestamp()),
            schema::variant::archived.eq(archived.map(seconds)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a variant");
    last_id(conn)
}

fn insert_measure(
    conn: &mut DbConnection,
    project_id: i32,
    name: &str,
    units: &str,
    created: i64,
    archived: Option<i64>,
) -> (i32, MeasureUuid) {
    let uuid = MeasureUuid::new();
    diesel::insert_into(schema::measure::table)
        .values((
            schema::measure::uuid.eq(uuid),
            schema::measure::project_id.eq(project_id),
            schema::measure::name.eq(name),
            schema::measure::slug.eq(name),
            schema::measure::units.eq(units),
            schema::measure::created.eq(seconds(created)),
            schema::measure::modified.eq(seconds(created)),
            schema::measure::archived.eq(archived.map(seconds)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a measure");
    (last_id(conn), uuid)
}

/// A report on a head, testbed, and version, taken at `created` seconds, with one value of the
/// variant for each measure.
fn insert_report(
    conn: &mut DbConnection,
    project_id: i32,
    (head, testbed, version): (i32, i32, i32),
    created: i64,
    (variant, measures): (i32, &[i32]),
) -> i32 {
    diesel::insert_into(schema::report::table)
        .values((
            schema::report::uuid.eq(ReportUuid::new()),
            schema::report::project_id.eq(project_id),
            schema::report::head_id.eq(head),
            schema::report::version_id.eq(version),
            schema::report::testbed_id.eq(testbed),
            schema::report::adapter.eq(0),
            schema::report::start_time.eq(seconds(created - 60)),
            schema::report::end_time.eq(seconds(created - 1)),
            schema::report::created.eq(seconds(created)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a report");
    let report = last_id(conn);
    let report_benchmark = insert_report_benchmark(conn, report, variant);
    for &measure in measures {
        diesel::insert_into(schema::metric::table)
            .values((
                schema::metric::uuid.eq(MetricUuid::new()),
                schema::metric::report_benchmark_id.eq(report_benchmark),
                schema::metric::measure_id.eq(measure),
                schema::metric::name.eq(MetricName::value()),
                schema::metric::value.eq(1.0),
            ))
            .execute(&mut *conn)
            .expect("Failed to insert a metric");
    }
    report
}

/// The variant's results in the report, with no values.
fn insert_report_benchmark(conn: &mut DbConnection, report: i32, variant: i32) -> i32 {
    let benchmark = schema::variant::table
        .filter(schema::variant::id.eq(variant))
        .select(schema::variant::benchmark_id)
        .first::<i32>(&mut *conn)
        .expect("Failed to read the variant's benchmark");
    diesel::insert_into(schema::report_benchmark::table)
        .values((
            schema::report_benchmark::uuid.eq(ReportBenchmarkUuid::new()),
            schema::report_benchmark::report_id.eq(report),
            schema::report_benchmark::iteration.eq(0),
            schema::report_benchmark::benchmark_id.eq(benchmark),
            schema::report_benchmark::variant_id.eq(variant),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a report benchmark");
    last_id(conn)
}

/// Make the report a run that ended at `end` seconds, before the API took it.
fn end_report_at(conn: &mut DbConnection, report: i32, end: i64) {
    diesel::update(schema::report::table.filter(schema::report::id.eq(report)))
        .set((
            schema::report::start_time.eq(seconds(end - 59)),
            schema::report::end_time.eq(seconds(end)),
        ))
        .execute(conn)
        .expect("Failed to backfill the report");
}

fn insert_threshold(
    conn: &mut DbConnection,
    project_id: i32,
    (branch, testbed, measure): (i32, i32, i32),
) -> ThresholdUuid {
    let uuid = ThresholdUuid::new();
    diesel::insert_into(schema::threshold::table)
        .values((
            schema::threshold::uuid.eq(uuid),
            schema::threshold::project_id.eq(project_id),
            schema::threshold::branch_id.eq(branch),
            schema::threshold::testbed_id.eq(testbed),
            schema::threshold::measure_id.eq(measure),
            schema::threshold::created.eq(seconds(0)),
            schema::threshold::modified.eq(seconds(0)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a threshold");
    uuid
}

fn last_id(conn: &mut DbConnection) -> i32 {
    diesel::select(last_insert_rowid())
        .get_result::<i32>(conn)
        .expect("Failed to read the last inserted ID")
}

fn seconds(offset: i64) -> DateTime {
    DateTime::try_from(base_timestamp().timestamp() + offset).expect("Failed to build a time")
}

fn millis(offset: i64) -> i64 {
    (base_timestamp().timestamp() + offset) * 1000
}

fn names<'b>(body: &'b Value, dimension: &str) -> Vec<&'b str> {
    body[dimension]
        .as_array()
        .expect("Failed to read the rows")
        .iter()
        .map(|row| row["name"].as_str().expect("Failed to read a name"))
        .collect()
}

/// A row's thresholds and held thresholds.
async fn counts(
    server: &TestServer,
    f: &Fixture,
    dimension: &str,
    query: &str,
    name: &str,
) -> (u64, u64) {
    let (status, body) = get(server, &f.user.token, &f.slug, dimension, query).await;
    assert_eq!(status, StatusCode::OK, "{dimension} {query}");
    let row = body[dimension]
        .as_array()
        .expect("Failed to read the rows")
        .iter()
        .find(|row| row["name"] == name)
        .expect("Failed to find the row");
    (
        row["thresholds"]
            .as_u64()
            .expect("Failed to read thresholds"),
        row["held_thresholds"]
            .as_u64()
            .expect("Failed to read held thresholds"),
    )
}

/// The thresholds the public list shows as active or as archived, of the fixture's two that
/// start out active.
async fn listed_thresholds(server: &TestServer, f: &Fixture, archived: bool) -> Vec<ThresholdUuid> {
    let (status, body) = request(
        server,
        Some(&f.user.token),
        &format!(
            "/v0/projects/{}/thresholds?archived={archived}&per_page=255",
            f.slug
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "thresholds archived={archived}");
    let listed = body
        .as_array()
        .expect("Failed to read the thresholds")
        .iter()
        .map(|threshold| {
            threshold["uuid"]
                .as_str()
                .expect("Failed to read a UUID")
                .to_owned()
        })
        .collect::<Vec<_>>();
    [f.t_main, f.t_feature]
        .into_iter()
        .filter(|uuid| listed.contains(&uuid.to_string()))
        .collect()
}

async fn patch_archived(server: &TestServer, f: &Fixture, path: &str, archived: bool) {
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/projects/{}/{path}", f.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&f.user.token),
        )
        .json(&json!({ "archived": archived }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "{path} {archived}");
}

async fn get(
    server: &TestServer,
    token: &str,
    slug: &str,
    dimension: &str,
    query: &str,
) -> (StatusCode, Value) {
    request(
        server,
        Some(token),
        &format!("/v0/projects/{slug}/console/{dimension}?{query}"),
    )
    .await
}

async fn request(server: &TestServer, token: Option<&str>, path: &str) -> (StatusCode, Value) {
    let mut request = server.client.get(server.api_url(path));
    if let Some(token) = token {
        request = request.header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        );
    }
    let resp = request.send().await.expect("Request failed");
    let status = resp.status();
    let body = resp.json().await.unwrap_or(Value::Null);
    (status, body)
}

async fn create_key(server: &TestServer, user: &TestUser, slug: &str) -> String {
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{slug}/keys")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&json!({ "name": "dimension key" }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED, "project key");
    let key: JsonProjectKeyCreated = resp.json().await.expect("Failed to parse the key");
    key.key.as_ref().to_owned()
}
