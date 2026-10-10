#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! The console's pinned plots: `/v0/projects/{project}/console/plots`.

use bencher_api_tests::{TestServer, helpers::grant_project_role};
use bencher_json::{
    DateTime, DateTimeMillis, JsonPlot, JsonProjectKeyCreated, JsonReport, PlotUuid, ProjectSlug,
    project::{
        Visibility,
        console::{JsonConsolePerf, JsonConsolePlots},
    },
};
use bencher_rbac::project::Role;
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

/// The server's frozen clock.
const NOW: &str = "2024-03-01T00:00:00Z";
const DAY: i64 = 24 * 60 * 60;

fn at(time: &str) -> DateTime {
    serde_json::from_value(serde_json::json!(time)).expect("Failed to parse the time")
}

fn millis(time: &str) -> i64 {
    DateTimeMillis::from(at(time)).into()
}

struct Fixture {
    slug: ProjectSlug,
    token: String,
}

async fn fixture(server: &TestServer, label: &str) -> Fixture {
    let user = server
        .signup("Test User", &format!("consoleplots{label}@example.com"))
        .await;
    let org = server
        .create_org(&user, &format!("Console Plots Org {label}"))
        .await;
    let project = server
        .create_project(&user, &org, &format!("Console Plots Project {label}"))
        .await;
    Fixture {
        slug: project.slug,
        token: user.token,
    }
}

async fn send(
    server: &TestServer,
    method: http::Method,
    path: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, String) {
    let mut request = server.client.request(method, server.api_url(path));
    if let Some(token) = token {
        request = request.header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        );
    }
    if let Some(body) = body {
        request = request.json(&body);
    }
    let resp = request.send().await.expect("Request failed");
    let status = resp.status();
    (
        status,
        resp.text().await.expect("Failed to read the response"),
    )
}

/// A BMF v1 report of `alpha` and `beta` latency on main and localhost.
async fn post(server: &TestServer, fixture: &Fixture, time: &str, value: f64) -> JsonReport {
    let start_time = at(time);
    let end_time = DateTime::try_from(start_time.timestamp() + 60).expect("Failed to end the run");
    let latency = |value: f64| serde_json::json!({ "latency": { "value": value } });
    let results = serde_json::json!({
        "alpha": [{ "parameters": { "size": 1 }, "measures": latency(value) }],
        "beta": [{ "parameters": { "size": 1 }, "measures": latency(value * 2.0) }],
    });
    let (status, text) = send(
        server,
        http::Method::POST,
        &format!("/v0/projects/{}/reports", fixture.slug),
        Some(&fixture.token),
        Some(serde_json::json!({
            "branch": "main",
            "testbed": "localhost",
            "hash": format!("{:0>40x}", start_time.timestamp()),
            "start_time": start_time,
            "end_time": end_time,
            "results": [results.to_string()],
            "bmf_version": 1,
            "settings": { "adapter": "json" },
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "POST report at {time}: {text}");
    serde_json::from_str(&text).expect("Failed to parse the report")
}

/// A BMF v1 report of `alpha` and `beta` at two sizes, each with latency and
/// throughput reported as `value` and `p99`.
async fn post_variants(server: &TestServer, fixture: &Fixture, time: &str) -> JsonReport {
    let start_time = at(time);
    let end_time = DateTime::try_from(start_time.timestamp() + 60).expect("Failed to end the run");
    let variant = |size: u8, value: f64| {
        let metrics = serde_json::json!({ "value": value, "p99": value * 1.5 });
        serde_json::json!({
            "parameters": { "size": size },
            "measures": { "latency": metrics, "throughput": metrics },
        })
    };
    let results = serde_json::json!({
        "alpha": [variant(1, 1.0), variant(2, 2.0)],
        "beta": [variant(1, 3.0), variant(2, 4.0)],
    });
    let (status, text) = send(
        server,
        http::Method::POST,
        &format!("/v0/projects/{}/reports", fixture.slug),
        Some(&fixture.token),
        Some(serde_json::json!({
            "branch": "main",
            "testbed": "localhost",
            "hash": format!("{:0>40x}", start_time.timestamp()),
            "start_time": start_time,
            "end_time": end_time,
            "results": [results.to_string()],
            "bmf_version": 1,
            "settings": { "adapter": "json" },
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "POST report at {time}: {text}");
    serde_json::from_str(&text).expect("Failed to parse the report")
}

/// Reports on the first, the tenth, the twentieth, and the twenty-seventh of February.
async fn history(server: &TestServer, fixture: &Fixture) -> JsonReport {
    let mut last = None;
    for (time, value) in [
        ("2024-02-01T00:00:00Z", 1.0),
        ("2024-02-10T00:00:00Z", 2.0),
        ("2024-02-20T00:00:00Z", 3.0),
        ("2024-02-27T00:00:00Z", 4.0),
    ] {
        last = Some(post(server, fixture, time, value).await);
    }
    last.expect("a report")
}

fn benchmark(report: &JsonReport, name: &str) -> String {
    report
        .results
        .as_ref()
        .expect("results")
        .iter()
        .flatten()
        .find(|result| result.benchmark.name.as_ref() == name)
        .map(|result| result.benchmark.uuid.to_string())
        .expect("the report has the benchmark")
}

fn measure(report: &JsonReport) -> String {
    report
        .results
        .as_ref()
        .expect("results")
        .iter()
        .flatten()
        .flat_map(|result| result.measures.iter())
        .map(|measure| measure.measure.uuid.to_string())
        .next()
        .expect("the report has a measure")
}

fn measure_named(report: &JsonReport, name: &str) -> String {
    report
        .results
        .as_ref()
        .expect("results")
        .iter()
        .flatten()
        .flat_map(|result| result.measures.iter())
        .find(|measure| measure.measure.name.as_ref() == name)
        .map(|measure| measure.measure.uuid.to_string())
        .expect("the report has the measure")
}

/// Pin `benchmark` on top of Plots, drawn over `days`, with any `extra` fields.
async fn pin(
    server: &TestServer,
    fixture: &Fixture,
    report: &JsonReport,
    title: &str,
    benchmark_name: &str,
    days: i64,
    extra: serde_json::Value,
) -> JsonPlot {
    let mut body = serde_json::json!({
        "title": title,
        "lower_value": false,
        "upper_value": false,
        "lower_boundary": false,
        "upper_boundary": false,
        "x_axis": "date_time",
        "window": days * DAY,
        "branches": [report.branch.uuid],
        "testbeds": [report.testbed.uuid],
        "benchmarks": [benchmark(report, benchmark_name)],
        "measures": [measure(report)],
    });
    if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
        body.extend(extra.clone());
    }
    let (status, text) = send(
        server,
        http::Method::POST,
        &format!("/v0/projects/{}/plots", fixture.slug),
        Some(&fixture.token),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "POST plot {title}: {text}");
    serde_json::from_str(&text).expect("Failed to parse the plot")
}

async fn try_plots(
    server: &TestServer,
    slug: &ProjectSlug,
    query: &str,
    token: Option<&str>,
) -> (StatusCode, String) {
    send(
        server,
        http::Method::GET,
        &format!("/v0/projects/{slug}/console/plots{query}"),
        token,
        None,
    )
    .await
}

async fn plots(server: &TestServer, fixture: &Fixture, query: &str) -> JsonConsolePlots {
    let (status, text) = try_plots(server, &fixture.slug, query, Some(&fixture.token)).await;
    assert_eq!(status, StatusCode::OK, "GET console plots{query}: {text}");
    serde_json::from_str(&text).expect("Failed to parse the console plots")
}

fn x(perf: &JsonConsolePerf) -> Vec<i64> {
    perf.points.x.iter().copied().map(i64::from).collect()
}

fn window(perf: &JsonConsolePerf) -> (i64, i64) {
    (perf.window.start_time.into(), perf.window.end_time.into())
}

fn titles(plots: &JsonConsolePlots) -> Vec<String> {
    plots
        .order
        .iter()
        .map(|plot| {
            plot.title
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default()
        })
        .collect()
}

fn uuids(plots: &JsonConsolePlots) -> Vec<PlotUuid> {
    plots.plots.iter().map(|tile| tile.plot.uuid).collect()
}

// Kills: one window for every plot, a window read from another plot, and a page
// out of the plots' order.
#[tokio::test]
async fn console_plots_draw_each_plot_at_its_own_window() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "own").await;
    let report = history(&server, &fixture).await;
    let month = pin(
        &server,
        &fixture,
        &report,
        "Month",
        "alpha",
        28,
        serde_json::json!({}),
    )
    .await;
    let week = pin(
        &server,
        &fixture,
        &report,
        "Week",
        "alpha",
        7,
        serde_json::json!({}),
    )
    .await;

    let response = plots(&server, &fixture, "").await;

    assert_eq!(uuids(&response), vec![week.uuid, month.uuid]);
    let [week_tile, month_tile] = &response.plots[..] else {
        panic!("two plots");
    };
    assert_eq!(
        window(&week_tile.perf),
        (millis(NOW) - 7 * DAY * 1000, millis(NOW))
    );
    assert_eq!(x(&week_tile.perf), vec![millis("2024-02-27T00:00:00Z")]);
    assert_eq!(
        window(&month_tile.perf),
        (millis(NOW) - 28 * DAY * 1000, millis(NOW))
    );
    assert_eq!(
        x(&month_tile.perf),
        vec![
            millis("2024-02-10T00:00:00Z"),
            millis("2024-02-20T00:00:00Z"),
            millis("2024-02-27T00:00:00Z"),
        ]
    );
    assert_eq!(
        week_tile.perf.lines[0].series.y,
        vec![Some(4.0)],
        "the plot draws its own benchmark"
    );
}

// Kills: a page window that only some plots draw, an end that is ignored
// without a start, and a start that is not each plot's own before that end.
#[tokio::test]
async fn console_plots_page_window_draws_every_plot() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "page").await;
    let report = history(&server, &fixture).await;
    pin(
        &server,
        &fixture,
        &report,
        "Month",
        "alpha",
        28,
        serde_json::json!({}),
    )
    .await;
    pin(
        &server,
        &fixture,
        &report,
        "Week",
        "alpha",
        7,
        serde_json::json!({}),
    )
    .await;

    let start = millis("2024-02-15T00:00:00Z");
    let end = millis("2024-02-25T00:00:00Z");
    let both = plots(
        &server,
        &fixture,
        &format!("?start_time={start}&end_time={end}"),
    )
    .await;
    for tile in &both.plots {
        assert_eq!(window(&tile.perf), (start, end));
        assert_eq!(x(&tile.perf), vec![millis("2024-02-20T00:00:00Z")]);
    }

    let end = millis("2024-02-21T00:00:00Z");
    let ending = plots(&server, &fixture, &format!("?end_time={end}")).await;
    let [week, month] = &ending.plots[..] else {
        panic!("two plots");
    };
    assert_eq!(window(&week.perf), (end - 7 * DAY * 1000, end));
    assert_eq!(x(&week.perf), vec![millis("2024-02-20T00:00:00Z")]);
    assert_eq!(window(&month.perf), (end - 28 * DAY * 1000, end));
    assert_eq!(
        x(&month.perf),
        vec![
            millis("2024-02-01T00:00:00Z"),
            millis("2024-02-10T00:00:00Z"),
            millis("2024-02-20T00:00:00Z"),
        ]
    );

    let (status, _) = try_plots(
        &server,
        &fixture.slug,
        &format!("?start_time={end}&end_time={start}"),
        Some(&fixture.token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "a start after the end");
}

// Kills: a page that does not start where the one before it ended, an order that
// is not the plots' own or holds only the page, and an empty page that draws
// the default page instead.
#[tokio::test]
async fn console_plots_pages_hold_their_plots_and_order_every_plot() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "pages").await;
    let report = history(&server, &fixture).await;
    let third = pin(
        &server,
        &fixture,
        &report,
        "Third",
        "alpha",
        7,
        serde_json::json!({}),
    )
    .await;
    let second = pin(
        &server,
        &fixture,
        &report,
        "Second",
        "alpha",
        7,
        serde_json::json!({}),
    )
    .await;
    pin(
        &server,
        &fixture,
        &report,
        "First",
        "alpha",
        7,
        serde_json::json!({}),
    )
    .await;

    let page = plots(&server, &fixture, "?per_page=2&page=2").await;
    assert_eq!(uuids(&page), vec![third.uuid]);
    assert_eq!(titles(&page), vec!["First", "Second", "Third"]);

    let (status, text) = send(
        &server,
        http::Method::PATCH,
        &format!("/v0/projects/{}/plots/{}", fixture.slug, second.uuid),
        Some(&fixture.token),
        Some(serde_json::json!({ "index": 2 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{text}");

    let order = plots(&server, &fixture, "?per_page=0").await;
    assert!(order.plots.is_empty());
    assert_eq!(titles(&order), vec!["First", "Third", "Second"]);
    assert_eq!(order.order[2].uuid, second.uuid);
}

// Kills: a plot's saved fields or lines taken from another plot on the page, and
// saved fields the page leaves out.
#[tokio::test]
async fn console_plots_draw_each_plot_as_saved() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "saved").await;
    let report = history(&server, &fixture).await;
    let beta = pin(
        &server,
        &fixture,
        &report,
        "Beta",
        "beta",
        28,
        serde_json::json!({
            "y_axis": "log",
            "layout": "stacked",
            "metrics": ["value"],
            "hidden": ["hiddenkey"],
            "focus": "focuskey",
        }),
    )
    .await;
    let alpha = pin(
        &server,
        &fixture,
        &report,
        "Alpha",
        "alpha",
        28,
        serde_json::json!({}),
    )
    .await;

    let response = plots(&server, &fixture, "").await;

    let [alpha_tile, beta_tile] = &response.plots[..] else {
        panic!("two plots");
    };
    for (tile, saved) in [(alpha_tile, &alpha), (beta_tile, &beta)] {
        assert_eq!(
            serde_json::to_value(&tile.plot).expect("a plot"),
            serde_json::to_value(saved).expect("a plot"),
        );
    }
    let names = |perf: &JsonConsolePerf| {
        perf.benchmarks
            .iter()
            .map(|benchmark| benchmark.name.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&alpha_tile.perf), vec!["alpha"]);
    assert_eq!(names(&beta_tile.perf), vec!["beta"]);
    assert_eq!(
        beta_tile.perf.lines[0].series.y,
        vec![Some(4.0), Some(6.0), Some(8.0)]
    );
}

// Kills: a pin's parameters filter, metrics, benchmark order, or measure order
// dropped from the lines it draws or from the saved fields it answers with.
#[tokio::test]
async fn console_plots_draw_each_plot_through_its_filters() {
    let server = TestServer::new_at(at(NOW)).await;
    let fixture = fixture(&server, "filters").await;
    let report = post_variants(&server, &fixture, "2024-02-27T00:00:00Z").await;
    let pinned = pin(
        &server,
        &fixture,
        &report,
        "Filtered",
        "beta",
        28,
        serde_json::json!({
            "benchmarks": [benchmark(&report, "beta"), benchmark(&report, "alpha")],
            "measures": [measure_named(&report, "Throughput"), measure_named(&report, "Latency")],
            "parameters": [{ "size": 2 }],
            "metrics": ["p99"],
        }),
    )
    .await;

    let response = plots(&server, &fixture, "").await;

    let [tile] = &response.plots[..] else {
        panic!("one plot");
    };
    assert_eq!(
        serde_json::to_value(&tile.plot).expect("a plot"),
        serde_json::to_value(&pinned).expect("a plot"),
    );
    let perf = &tile.perf;
    let named = |index: u32, names: &[String]| {
        usize::try_from(index)
            .ok()
            .and_then(|index| names.get(index))
            .cloned()
            .expect("an index into the answer")
    };
    let benchmarks = perf
        .benchmarks
        .iter()
        .map(|benchmark| benchmark.name.to_string())
        .collect::<Vec<_>>();
    let variants = perf
        .variants
        .iter()
        .map(|variant| serde_json::to_string(&variant.parameters).expect("parameters"))
        .collect::<Vec<_>>();
    let measures = perf
        .measures
        .iter()
        .map(|measure| measure.name.to_string())
        .collect::<Vec<_>>();
    let mut lines = perf
        .lines
        .iter()
        .map(|line| {
            (
                named(line.benchmark, &benchmarks),
                named(line.variant, &variants),
                named(line.measure, &measures),
                line.metric.to_string(),
            )
        })
        .collect::<Vec<_>>();
    lines.sort();
    let line = |benchmark: &str, measure: &str| {
        (
            benchmark.to_owned(),
            r#"{"size":2}"#.to_owned(),
            measure.to_owned(),
            "p99".to_owned(),
        )
    };
    assert_eq!(
        lines,
        vec![
            line("alpha", "Latency"),
            line("alpha", "Throughput"),
            line("beta", "Latency"),
            line("beta", "Throughput"),
        ]
    );
}

// Kills: plots served to a reader who is not signed in, to a stranger or another
// project's key on a private project, or to another project's key on a public
// one; plots refused to a viewer or to the project's own key; and a refusal that
// names a private pin.
#[tokio::test]
async fn console_plots_need_a_signed_in_reader() {
    use StatusCode as S;

    let server = TestServer::new_at(at(NOW)).await;
    let public = fixture(&server, "public").await;
    let private = fixture(&server, "private").await;
    let report = post(&server, &private, "2024-02-27T00:00:00Z", 1.0).await;
    pin(
        &server,
        &private,
        &report,
        "Secret pin",
        "alpha",
        7,
        serde_json::json!({}),
    )
    .await;
    diesel::update(
        schema::project::table.filter(schema::project::slug.eq(private.slug.to_string())),
    )
    .set(schema::project::visibility.eq(Visibility::Private))
    .execute(&mut server.db_conn())
    .expect("Failed to make the project private");
    let other = fixture(&server, "other").await;
    let viewer = server
        .signup("Plots Viewer", "consoleplotsviewer@example.com")
        .await;
    grant_project_role(&server, &viewer, public.slug.as_ref(), Role::Viewer);
    grant_project_role(&server, &viewer, private.slug.as_ref(), Role::Viewer);
    let stranger = server
        .signup("Plots Stranger", "consoleplotsstranger@example.com")
        .await;
    let public_key = create_key(&server, &public).await;
    let private_key = create_key(&server, &private).await;
    let other_key = create_key(&server, &other).await;

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
            let (status, text) = try_plots(&server, &f.slug, "", token).await;
            assert_eq!(status, expected, "{} {token:?}: {text}", f.slug);
            if status != S::OK {
                assert!(!text.contains("Secret pin"), "{} {token:?}: {text}", f.slug);
            }
        }
    }
}

async fn create_key(server: &TestServer, fixture: &Fixture) -> String {
    let (status, text) = send(
        server,
        http::Method::POST,
        &format!("/v0/projects/{}/keys", fixture.slug),
        Some(&fixture.token),
        Some(serde_json::json!({ "name": "console-key" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create a project key: {text}");
    let key: JsonProjectKeyCreated = serde_json::from_str(&text).expect("a project key");
    key.key.as_ref().to_owned()
}
