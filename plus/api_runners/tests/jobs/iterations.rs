//! A job stores one iteration per run of its benchmark, numbered as the runner numbered them,
//! with the files of each run merged into it.

use std::collections::BTreeMap;

use bencher_api_tests::TestServer;
use bencher_json::{
    JobStatus, ParameterSet,
    project::report::{ReportWarningAction, ReportWarningResource},
    runner::{JsonIterationOutput, JsonJobOutput},
};
use bencher_schema::{model::runner::reprocess_completed_jobs, schema};
use camino::Utf8PathBuf;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

use super::{
    common::{
        create_test_report, get_project_id, insert_test_job_with_bmf_version, insert_test_spec,
        set_job_status,
    },
    get_status,
};

/// One stored value: its iteration, benchmark, measure, metric name, and value.
type Stored = (i32, String, String, String, f64);

/// A job with 2 files and 3 runs stores iterations 0, 1, and 2, and each holds the benchmarks
/// of both files.
///
/// Kills: a job that stores 1 iteration per file, as 6 iterations numbered 0 to 5.
#[tokio::test]
async fn job_stores_one_iteration_per_run() {
    let results = (1..=3)
        .map(|run| {
            files(&[
                ("/output/a.json", v0("bench_a", "latency", f64::from(run))),
                (
                    "/output/b.json",
                    v0("bench_b", "latency", f64::from(run * 10)),
                ),
            ])
        })
        .collect();
    let (server, report_id) = process_job(
        "runs",
        serde_json::json!({ "file_paths": ["/output/a.json", "/output/b.json"] }),
        results,
    )
    .await;

    assert_eq!(
        stored(&server, report_id),
        vec![
            value(0, "bench_a", "latency", 1.0),
            value(0, "bench_b", "latency", 10.0),
            value(1, "bench_a", "latency", 2.0),
            value(1, "bench_b", "latency", 20.0),
            value(2, "bench_a", "latency", 3.0),
            value(2, "bench_b", "latency", 30.0),
        ]
    );
    assert_eq!(warnings(&server, report_id), Vec::new());
}

/// A BMF v0 measure that 2 files of 1 run report takes the later file in the job's file order,
/// whole, and the report counts each replaced value.
///
/// The paths sort in the reverse of the file order, so a merge in the order of the runner's
/// output map stores the earlier file's value.
///
/// Kills: a merge in path order, a merge that keeps the earlier file, a v0 merge that keeps the
/// earlier file's bounds beside the later file's value, and a replaced value with no warning.
#[tokio::test]
async fn job_repeated_v0_measure_takes_the_later_file() {
    let results = (1..=2)
        .map(|run| {
            let run = f64::from(run);
            files(&[
                (
                    "/output/z.json",
                    serde_json::json!({ "bench": { "latency": {
                        "value": 10.0 + run,
                        "lower_value": 9.0 + run,
                        "upper_value": 11.0 + run,
                    } } })
                    .to_string(),
                ),
                ("/output/a.json", v0("bench", "latency", 20.0 + run)),
            ])
        })
        .collect();
    let (server, report_id) = process_job(
        "repeat-v0",
        serde_json::json!({ "file_paths": ["/output/z.json", "/output/a.json"] }),
        results,
    )
    .await;

    assert_eq!(
        stored(&server, report_id),
        vec![
            value(0, "bench", "latency", 21.0),
            value(1, "bench", "latency", 22.0),
        ]
    );
    assert_eq!(
        warnings(&server, report_id),
        vec![(
            ReportWarningResource::Metric,
            ReportWarningAction::Ignore,
            2
        )]
    );
}

/// A BMF v1 metric name that 2 files of 1 run report takes the later file in the job's file
/// order, and the names only one file reports are all kept.
///
/// The later file repeats 1 name and adds 2, so the count of replaced values tells the two
/// apart.
///
/// Kills: a v1 merge keyed by the measure alone, which drops the earlier file's `p99`, a merge
/// in path order, and a count of the names the later file adds rather than repeats.
#[tokio::test]
async fn job_repeated_v1_metric_takes_the_later_file() {
    let results = vec![files(&[
        (
            "/output/z.json",
            serde_json::json!({ "bench": [{ "measures": {
                "latency": { "value": 10.0, "p99": 30.0 },
            } }] })
            .to_string(),
        ),
        (
            "/output/a.json",
            serde_json::json!({ "bench": [{ "measures": {
                "latency": { "value": 20.0, "p50": 15.0, "p90": 18.0 },
            } }] })
            .to_string(),
        ),
    ])];
    let (server, report_id) = process_job(
        "repeat-v1",
        serde_json::json!({
            "file_paths": ["/output/z.json", "/output/a.json"],
            "bmf_version": 1,
        }),
        results,
    )
    .await;

    assert_eq!(
        stored(&server, report_id),
        vec![
            metric(0, "bench", "latency", "p50", 15.0),
            metric(0, "bench", "latency", "p90", 18.0),
            metric(0, "bench", "latency", "p99", 30.0),
            metric(0, "bench", "latency", "value", 20.0),
        ]
    );
    assert_eq!(
        warnings(&server, report_id),
        vec![(
            ReportWarningResource::Metric,
            ReportWarningAction::Ignore,
            1
        )]
    );
}

/// 2 files that report different measures of 1 benchmark in 1 run both store into that run.
///
/// Kills: a merge that replaces a benchmark's measures wholesale, and a job that stores each
/// file as its own iteration.
#[tokio::test]
async fn job_different_measures_of_a_benchmark_are_both_stored() {
    let results = vec![files(&[
        ("/output/a.json", v0("bench", "latency", 1.0)),
        ("/output/b.json", v0("bench", "throughput", 2.0)),
    ])];
    let (server, report_id) = process_job(
        "measures",
        serde_json::json!({ "file_paths": ["/output/a.json", "/output/b.json"] }),
        results,
    )
    .await;

    assert_eq!(
        stored(&server, report_id),
        vec![
            value(0, "bench", "latency", 1.0),
            value(0, "bench", "throughput", 2.0),
        ]
    );
    assert_eq!(warnings(&server, report_id), Vec::new());
}

/// 2 files that report 1 benchmark at different BMF v1 parameters in 1 run both store into that
/// run, each at its own parameters.
///
/// Kills: a merge keyed without the parameters, which stores the later file's variant at the
/// parameters of another, or replaces the earlier file's variant.
#[tokio::test]
async fn job_different_parameters_of_a_benchmark_are_both_stored() {
    let payload = |n: u32, latency: f64| {
        serde_json::json!({ "bench": [{
            "parameters": { "n": n },
            "measures": { "latency": { "value": latency } },
        }] })
        .to_string()
    };
    let results = vec![files(&[
        ("/output/a.json", payload(1, 1.0)),
        ("/output/b.json", payload(2, 2.0)),
    ])];
    let (server, report_id) = process_job(
        "parameters",
        serde_json::json!({
            "file_paths": ["/output/a.json", "/output/b.json"],
            "bmf_version": 1,
        }),
        results,
    )
    .await;

    assert_eq!(
        stored_variants(&server, report_id),
        vec![
            (0, r#"{"n":1}"#.to_owned(), 1.0),
            (0, r#"{"n":2}"#.to_owned(), 2.0),
        ]
    );
    assert_eq!(warnings(&server, report_id), Vec::new());
}

/// A job with 1 file per run stores 1 iteration per run with that file's values, as before.
#[tokio::test]
async fn job_one_file_per_run_is_unchanged() {
    let results = (1..=2)
        .map(|run| files(&[("/output/a.json", v0("bench", "latency", f64::from(run)))]))
        .collect();
    let (server, report_id) = process_job(
        "one-file",
        serde_json::json!({ "file_paths": ["/output/a.json"] }),
        results,
    )
    .await;

    assert_eq!(
        stored(&server, report_id),
        vec![
            value(0, "bench", "latency", 1.0),
            value(1, "bench", "latency", 2.0),
        ]
    );
    assert_eq!(warnings(&server, report_id), Vec::new());
}

/// A BMF v0 fold of a job with 2 files and 3 runs folds the 3 runs.
///
/// Kills: a fold over the 6 files, whose mean divides each benchmark's sum by 6.
#[tokio::test]
async fn job_fold_folds_over_the_runs() {
    let results = (1..=3)
        .map(|run| {
            files(&[
                ("/output/a.json", v0("bench_a", "latency", f64::from(run))),
                (
                    "/output/b.json",
                    v0("bench_b", "latency", f64::from(run * 10)),
                ),
            ])
        })
        .collect();
    let (server, report_id) = process_job(
        "fold",
        serde_json::json!({
            "file_paths": ["/output/a.json", "/output/b.json"],
            "fold": "mean",
        }),
        results,
    )
    .await;

    assert_eq!(
        stored(&server, report_id),
        vec![
            value(0, "bench_a", "latency", 2.0),
            value(0, "bench_b", "latency", 20.0),
        ]
    );
}

/// A job with no output files stores each run's stdout as that run, as before.
#[tokio::test]
async fn job_stdout_only_is_unchanged() {
    let results = (1..=2)
        .map(|run| stdout(&v0("bench", "latency", f64::from(run))))
        .collect();
    let (server, report_id) = process_job("stdout", serde_json::json!({}), results).await;

    assert_eq!(
        stored(&server, report_id),
        vec![
            value(0, "bench", "latency", 1.0),
            value(1, "bench", "latency", 2.0),
        ]
    );
    assert_eq!(warnings(&server, report_id), Vec::new());
}

/// A run that returned no results, as a build time run that failed does, keeps its number, so
/// the run after it stores as the runner numbered it.
///
/// Kills: a job that numbers only the runs with results, which pairs the wrong runs of 2 jobs.
#[tokio::test]
async fn job_run_with_no_results_keeps_its_number() {
    let mut failed = stdout("the build failed");
    failed.output = Some(BTreeMap::new());
    let results = vec![
        stdout(&v0("bench", "latency", 1.0)),
        failed,
        stdout(&v0("bench", "latency", 3.0)),
    ];
    let (server, report_id) = process_job("no-results", serde_json::json!({}), results).await;

    assert_eq!(
        stored(&server, report_id),
        vec![
            value(0, "bench", "latency", 1.0),
            value(2, "bench", "latency", 3.0),
        ]
    );
}

/// Store `results` as a completed job's output, with `config` added to the test job's config,
/// process it, and return the server and the job's report.
#[expect(clippy::expect_used, reason = "test helper")]
async fn process_job(
    label: &str,
    config: serde_json::Value,
    results: Vec<JsonIterationOutput>,
) -> (TestServer, i32) {
    let server = TestServer::new().await;
    let admin = server
        .signup("Admin", &format!("iterations-{label}@example.com"))
        .await;
    let org = server
        .create_org(&admin, &format!("Iterations {label}"))
        .await;
    let project = server
        .create_project(&admin, &org, &format!("Iterations {label}"))
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);
    let job_uuid =
        insert_test_job_with_bmf_version(&server, report_id, project.uuid, spec_id, None);
    let mut job_config: serde_json::Value = serde_json::from_str(
        &schema::job::table
            .filter(schema::job::uuid.eq(job_uuid))
            .select(schema::job::config)
            .first::<String>(&mut server.db_conn())
            .expect("Failed to load the job config"),
    )
    .expect("Failed to parse the job config");
    let object = job_config
        .as_object_mut()
        .expect("The job config is an object");
    if let serde_json::Value::Object(added) = config {
        object.extend(added);
    }
    diesel::update(schema::job::table.filter(schema::job::uuid.eq(job_uuid)))
        .set(schema::job::config.eq(job_config.to_string()))
        .execute(&mut server.db_conn())
        .expect("Failed to set the job config");
    set_job_status(&server, job_uuid, JobStatus::Completed);

    let output = JsonJobOutput {
        results,
        error: None,
    };
    server
        .context()
        .oci_storage()
        .job_output()
        .put(project.uuid, job_uuid, &output)
        .await
        .expect("Failed to store job output");

    let log = slog::Logger::root(slog::Discard, slog::o!());
    reprocess_completed_jobs(&log, server.context()).await;
    assert_eq!(
        get_status(&server, job_uuid),
        JobStatus::Processed,
        "expected the job's results to process"
    );

    (server, report_id)
}

/// Every value the report stored, in order.
#[expect(clippy::expect_used, reason = "test helper")]
fn stored(server: &TestServer, report_id: i32) -> Vec<Stored> {
    schema::metric::table
        .inner_join(schema::report_benchmark::table.inner_join(schema::benchmark::table))
        .inner_join(schema::measure::table)
        .filter(schema::report_benchmark::report_id.eq(report_id))
        .select((
            schema::report_benchmark::iteration,
            schema::benchmark::name,
            schema::measure::slug,
            schema::metric::name,
            schema::metric::value,
        ))
        .order((
            schema::report_benchmark::iteration,
            schema::benchmark::name,
            schema::measure::slug,
            schema::metric::name,
        ))
        .load(&mut server.db_conn())
        .expect("Failed to load the stored values")
}

/// Every value the report stored, as its iteration, its canonical parameters, and its value,
/// in order.
#[expect(clippy::expect_used, reason = "test helper")]
fn stored_variants(server: &TestServer, report_id: i32) -> Vec<(i32, String, f64)> {
    let rows: Vec<(i32, ParameterSet, f64)> = schema::metric::table
        .inner_join(schema::report_benchmark::table.inner_join(schema::variant::table))
        .filter(schema::report_benchmark::report_id.eq(report_id))
        .select((
            schema::report_benchmark::iteration,
            schema::variant::parameters,
            schema::metric::value,
        ))
        .order((schema::report_benchmark::iteration, schema::metric::value))
        .load(&mut server.db_conn())
        .expect("Failed to load the stored variants");
    rows.into_iter()
        .map(|(iteration, parameters, value)| (iteration, parameters.canonical(), value))
        .collect()
}

/// Every warning the report stored.
#[expect(clippy::expect_used, reason = "test helper")]
fn warnings(
    server: &TestServer,
    report_id: i32,
) -> Vec<(ReportWarningResource, ReportWarningAction, i32)> {
    schema::report_warning::table
        .filter(schema::report_warning::report_id.eq(report_id))
        .select((
            schema::report_warning::resource,
            schema::report_warning::action,
            schema::report_warning::count,
        ))
        .load(&mut server.db_conn())
        .expect("Failed to load the warnings")
}

fn value(iteration: i32, benchmark: &str, measure: &str, value: f64) -> Stored {
    metric(iteration, benchmark, measure, "value", value)
}

fn metric(iteration: i32, benchmark: &str, measure: &str, name: &str, value: f64) -> Stored {
    (
        iteration,
        benchmark.to_owned(),
        measure.to_owned(),
        name.to_owned(),
        value,
    )
}

/// A BMF v0 payload of one benchmark with one measure.
fn v0(benchmark: &str, measure: &str, value: f64) -> String {
    serde_json::json!({ benchmark: { measure: { "value": value } } }).to_string()
}

/// A run that wrote `files`, each a path and its contents.
fn files(files: &[(&str, String)]) -> JsonIterationOutput {
    JsonIterationOutput {
        exit_code: 0,
        stdout: None,
        stderr: None,
        output: Some(
            files
                .iter()
                .map(|(path, contents)| (Utf8PathBuf::from(*path), contents.clone()))
                .collect(),
        ),
    }
}

/// A run that wrote no file and printed `stdout`.
fn stdout(stdout: &str) -> JsonIterationOutput {
    JsonIterationOutput {
        exit_code: 0,
        stdout: Some(stdout.to_owned()),
        stderr: None,
        output: None,
    }
}
