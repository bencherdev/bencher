//! The BMF version a report reads and declares, and the parts and thresholds it builds at it.

use super::{
    ATTEMPTS, CheckRuns, JobRead, PROJECT, ProjectRead, Run, Scene, StandIn, UUID, VERSION_PATH,
    VersionRead, bencher_in_scene,
};

const PROJECT_PATH: &str = "/v0/projects/project";
const RUN_PATH: &str = "/v0/run";
const REPORTS_PATH: &str = "/v0/projects/project/reports";
const FILE: [&str; 2] = ["--file", "Cargo.toml"];
const MARKER: &str = "benchmark-ran-marker";
const MARKER_COMMAND: &str = "echo benchmark-ran-marker";
const PLAIN: [&str; 6] = [
    "--threshold-measure",
    "latency",
    "--threshold-test",
    "t_test",
    "--threshold-upper-boundary",
    "0.99",
];
const METRIC: [&str; 2] = ["--threshold-metric", "p99"];
const REPORT: [&str; 12] = [
    "--key",
    "bencher_run_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh",
    "--branch",
    "main",
    "--testbed",
    "base",
    "--start-time",
    "2026-01-01T00:00:00Z",
    "--end-time",
    "2026-01-01T00:01:00Z",
    "--results",
    "{}",
];

// Kills: a run that reads no version or declares none, a project that does not exist read as
// anything but version 1, a server without the field read as anything but version 0 or as a
// failure, and an `--average` held back at version 1.
#[test]
fn a_run_declares_the_version_its_project_reads_at() {
    for (project, declared) in [
        (ProjectRead::Version(0), 0),
        (ProjectRead::Version(1), 1),
        (ProjectRead::NotFound, 1),
        (ProjectRead::NoVersion, 0),
    ] {
        let run = run_on(
            project,
            &[&ATTEMPTS[..], &PROJECT, &FILE, &["--average", "median"]].concat(),
        );
        run.assert_success();
        assert_eq!(run.requests_to("GET", PROJECT_PATH).len(), 1);
        let sent = run.body("POST", RUN_PATH);
        assert_eq!(sent["bmf_version"], declared, "{sent}");
        assert_eq!(sent["settings"]["average"], "median", "{sent}");
    }
}

// Kills: a project that does not exist read as a failure when the server's version differs from
// the CLI's, so its not found answer carries a version mismatch hint.
#[test]
fn a_project_that_does_not_exist_reads_as_version_1_on_a_server_of_another_version() {
    let run = bencher_in_scene(
        StandIn {
            version: VersionRead::Version("0.0.1"),
            ..stand_in()
        },
        Scene {
            project: ProjectRead::NotFound,
            ..Scene::default()
        },
        None,
        &[],
        &["run"],
        &[&ATTEMPTS[..], &PROJECT, &FILE].concat(),
    );
    run.assert_success();
    assert_eq!(run.requests_to("GET", PROJECT_PATH).len(), 1);
    assert_eq!(run.body("POST", RUN_PATH)["bmf_version"], 1);
}

// Kills: a read that fails over to declaring nothing or a version, and a run that sends its
// report after the read fails.
#[test]
fn a_failed_read_stops_the_run() {
    for scene in [
        Scene {
            project: ProjectRead::Forbidden,
            ..Scene::default()
        },
        Scene {
            project_failures: 1,
            ..Scene::default()
        },
    ] {
        let run = run_in(scene, &[&ATTEMPTS[..], &PROJECT, &FILE].concat());
        assert_posted_nothing(&run);
        let stderr = run.stderr();
        assert!(
            stderr.contains("Failed to read the BMF version of the project"),
            "{stderr}"
        );
    }
}

// Kills: a read that tries once, whatever `--attempts` says.
#[test]
fn the_read_retries_as_the_report_does() {
    let run = run_in(
        Scene {
            project: ProjectRead::Version(1),
            project_failures: 1,
            ..Scene::default()
        },
        &[&["--attempts", "2"][..], &PROJECT, &FILE].concat(),
    );
    run.assert_success();
    assert_eq!(run.requests_to("GET", PROJECT_PATH).len(), 2);
    assert_eq!(run.body("POST", RUN_PATH)["bmf_version"], 1);
}

// Kills: a build time or file size written at a version other than the one the run read.
#[test]
fn built_parts_are_written_at_the_version_read() {
    for (version, part) in [
        (0, &["--build-time", "true"][..]),
        (1, &["--build-time", "true"]),
        (0, &["--file-size", "Cargo.toml"]),
        (1, &["--file-size", "Cargo.toml"]),
    ] {
        let run = run_on(
            ProjectRead::Version(version),
            &[&ATTEMPTS[..], &PROJECT, part].concat(),
        );
        run.assert_success();
        let results = sent_results(&run);
        let (benchmark, measure) = if part.contains(&"--build-time") {
            ("/bin/sh -c true", "build-time")
        } else {
            ("Cargo.toml", "file-size")
        };
        let is_list = results
            .get(benchmark)
            .is_some_and(serde_json::Value::is_array);
        let names = if version == 1 {
            vec!["value"]
        } else {
            vec!["lower_value", "upper_value", "value"]
        };
        assert_eq!(is_list, version == 1, "{results}");
        assert_eq!(
            metric_names(&results, benchmark, measure),
            Some(names.into_iter().map(ToOwned::to_owned).collect()),
            "{results}"
        );
    }
}

// Kills: thresholds built without the version read, plain flags refused or listed at either
// version, and a metric at version 0 that is sent, or refused only after the benchmark runs.
#[test]
fn threshold_flags_are_built_at_the_version_read() {
    let model = serde_json::json!({ "test": "t_test", "upper_boundary": 0.99 });
    let listed = serde_json::json!([{ "measure": "latency", "metric": "p99", "model": model }]);
    let mapped = serde_json::json!({ "latency": model });
    for (version, flags, models) in [
        (0, &PLAIN[..], &mapped),
        (1, &PLAIN[..], &mapped),
        (1, &[&PLAIN[..], &METRIC].concat(), &listed),
    ] {
        let run = run_on(
            ProjectRead::Version(version),
            &[&ATTEMPTS[..], &PROJECT, flags, &FILE].concat(),
        );
        run.assert_success();
        let sent = run.body("POST", RUN_PATH);
        assert_eq!(&sent["thresholds"]["models"], models, "{sent}");
    }

    let run = run_on(
        ProjectRead::Version(0),
        &[&ATTEMPTS[..], &PROJECT, &PLAIN, &METRIC, &[MARKER_COMMAND]].concat(),
    );
    assert_posted_nothing(&run);
    let stderr = run.stderr();
    assert!(
        stderr.contains("`bencher project update --bmf-version 1`"),
        "{stderr}"
    );
    assert!(!run.stdout().contains(MARKER), "{}", run.stdout());
}

// Kills: a metric under `--adapter json_v0` that is sent, or refused with the move to version 1,
// which cannot help, in place of `--adapter json_v1`, by `bencher run` or `bencher report create`.
#[test]
fn a_metric_with_json_v0_is_refused_with_json_v1_as_its_fix() {
    let flags = [&["--adapter", "json_v0"][..], &PLAIN, &METRIC].concat();
    for run in [
        run_on(
            ProjectRead::Version(1),
            &[&ATTEMPTS[..], &PROJECT, &flags, &[MARKER_COMMAND]].concat(),
        ),
        report_create(ProjectRead::Version(1), &flags),
    ] {
        assert_posted_nothing(&run);
        let stderr = run.stderr();
        assert!(
            stderr.contains(
                "The latency Measure Threshold has a metric or parameters, which need BMF version 1, but `--adapter json_v0` reads at BMF version 0. Use `--adapter json_v1`, or remove `--threshold-metric` and `--threshold-parameters`."
            ),
            "{stderr}"
        );
        assert!(!stderr.contains("bencher project update"), "{stderr}");
        assert!(!run.stdout().contains(MARKER), "{}", run.stdout());
    }
}

// Kills: an explicit JSON adapter whose version the project's overrides, a read that it needs,
// and an adapter the request does not name, for `bencher run` and `bencher report create`.
#[test]
fn an_explicit_json_adapter_declares_its_version_on_any_project() {
    for (project, adapter, declared) in [(1, "json_v0", 0), (0, "json_v1", 1)] {
        let flags = ["--adapter", adapter];
        for (run, path) in [
            (
                run_on(
                    ProjectRead::Version(project),
                    &[&ATTEMPTS[..], &PROJECT, &flags, &FILE].concat(),
                ),
                RUN_PATH,
            ),
            (
                report_create(ProjectRead::Version(project), &flags),
                REPORTS_PATH,
            ),
        ] {
            run.assert_success();
            assert!(run.requests_to("GET", PROJECT_PATH).is_empty(), "{path}");
            let sent = run.body("POST", path);
            assert_eq!(sent["bmf_version"], declared, "{sent}");
            assert_eq!(sent["settings"]["adapter"], adapter, "{sent}");
        }
    }
}

// Kills: a run without a project that reads any project, by its context or by its image, or
// that declares anything but version 1.
#[test]
fn a_run_without_a_project_reads_no_project_and_declares_version_1() {
    for args in [&FILE[..], &["--image", "project:v1", "--detach"]] {
        let run = run_on(ProjectRead::Version(0), &[&ATTEMPTS[..], args].concat());
        run.assert_success();
        assert_eq!(project_reads(&run), 0, "{args:?}");
        assert_eq!(run.body("POST", RUN_PATH)["bmf_version"], 1, "{args:?}");
    }
}

// Kills: a remote job that declares no version, or one other than its project's.
#[test]
fn an_image_run_declares_the_version_of_its_project() {
    let run = run_on(
        ProjectRead::Version(0),
        &[
            &ATTEMPTS[..],
            &PROJECT,
            &["--image", "alpine:3.18", "--detach"],
        ]
        .concat(),
    );
    run.assert_success();
    assert_eq!(run.requests_to("GET", PROJECT_PATH).len(), 1);
    assert_eq!(run.body("POST", RUN_PATH)["bmf_version"], 0);
}

// Kills: a report create that reads no version, declares none, drops its thresholds, or prints
// the project it read.
#[test]
fn report_create_declares_its_project_s_version_and_sends_its_thresholds() {
    let run = report_create(ProjectRead::Version(1), &[&PLAIN[..], &METRIC].concat());
    run.assert_success();
    assert_eq!(run.requests_to("GET", PROJECT_PATH).len(), 1);
    let sent = run.body("POST", REPORTS_PATH);
    assert_eq!(sent["bmf_version"], 1, "{sent}");
    assert_eq!(sent["thresholds"]["models"][0]["metric"], "p99", "{sent}");
    let printed: serde_json::Value =
        serde_json::from_str(&run.stdout()).expect("The command printed more than its report");
    assert_eq!(printed["uuid"], UUID, "{printed}");
}

// Kills: a version check that each request makes again, or one that the read's silenced backend
// does not share with the report's.
#[test]
fn a_run_and_a_report_create_check_the_server_version_once() {
    for run in [
        run_on(
            ProjectRead::Version(1),
            &[&ATTEMPTS[..], &PROJECT, &FILE].concat(),
        ),
        report_create(ProjectRead::Version(1), &[]),
    ] {
        run.assert_success();
        assert_eq!(run.requests_to("GET", PROJECT_PATH).len(), 1);
        assert_eq!(run.requests_to("GET", VERSION_PATH).len(), 1);
    }
}

// Kills: a version mismatch warning that prints only for the request that checks the version,
// which for report create is the silenced read.
#[test]
fn report_create_warns_of_a_server_of_another_version() {
    let run = bencher_in_scene(
        StandIn {
            version: VersionRead::Version("0.0.1"),
            ..stand_in()
        },
        Scene {
            project: ProjectRead::Version(1),
            ..Scene::default()
        },
        None,
        &[],
        &["report", "create", "project"],
        &[&ATTEMPTS[..], &REPORT].concat(),
    );
    run.assert_success();
    assert_eq!(run.requests_to("GET", VERSION_PATH).len(), 1);
    let stderr = run.stderr();
    assert!(
        stderr.contains("Warning: The Bencher API server version is 0.0.1"),
        "{stderr}"
    );
}

// Kills: a threshold missing its metric that is refused only after a request, by `bencher run` or
// `bencher report create`, so a CI check can start before the refusal.
#[test]
fn a_threshold_missing_its_metric_is_refused_before_any_request() {
    let flags = [
        &PLAIN[..],
        &METRIC,
        &[
            "--threshold-measure",
            "throughput",
            "--threshold-test",
            "t_test",
            "--threshold-upper-boundary",
            "0.99",
            "--threshold-metric",
            "_",
        ],
    ]
    .concat();
    for run in [
        run_on(
            ProjectRead::Version(1),
            &[&ATTEMPTS[..], &PROJECT, &flags, &[MARKER_COMMAND]].concat(),
        ),
        report_create(ProjectRead::Version(1), &flags),
    ] {
        assert!(!run.output.status.success(), "{}", run.stdout());
        assert!(run.requests.is_empty(), "{}", run.stderr());
        let stderr = run.stderr();
        assert!(
            stderr.contains("The throughput Measure Threshold is missing its metric"),
            "{stderr}"
        );
        assert!(!run.stdout().contains(MARKER), "{}", run.stdout());
    }
}

/// `bencher run` with `args` against a stand-in whose projects read as `project`.
fn run_on(project: ProjectRead, args: &[&str]) -> Run {
    run_in(
        Scene {
            project,
            ..Scene::default()
        },
        args,
    )
}

fn run_in(scene: Scene, args: &[&str]) -> Run {
    bencher_in_scene(stand_in(), scene, None, &[], &["run"], args)
}

/// `bencher report create` for `project` with `args` against a stand-in whose projects read as
/// `project_read`.
fn report_create(project_read: ProjectRead, args: &[&str]) -> Run {
    bencher_in_scene(
        stand_in(),
        Scene {
            project: project_read,
            ..Scene::default()
        },
        None,
        &[],
        &["report", "create", "project"],
        &[&ATTEMPTS[..], &REPORT, args].concat(),
    )
}

fn stand_in() -> StandIn {
    StandIn {
        job_read: JobRead::Callback("pending"),
        version: VersionRead::Current,
        check_runs: CheckRuns::Create,
    }
}

/// The one result the run sent, as JSON.
#[expect(clippy::expect_used, reason = "test helper")]
fn sent_results(run: &Run) -> serde_json::Value {
    let sent = run.body("POST", RUN_PATH);
    let result = sent
        .pointer("/results/0")
        .and_then(serde_json::Value::as_str)
        .expect("The run sent no results");
    serde_json::from_str(result).expect("The results are not JSON")
}

fn assert_posted_nothing(run: &Run) {
    assert!(!run.output.status.success(), "{}", run.stdout());
    assert!(
        run.requests.iter().all(|request| request.method != "POST"),
        "{}",
        run.stderr()
    );
}

/// The names of the metric that `results` holds for `measure` of `benchmark`.
fn metric_names(
    results: &serde_json::Value,
    benchmark: &str,
    measure: &str,
) -> Option<Vec<String>> {
    let benchmark = results.get(benchmark)?;
    let measures = benchmark
        .get(0)
        .and_then(|variant| variant.get("measures"))
        .unwrap_or(benchmark);
    let mut names = measures
        .get(measure)?
        .as_object()?
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    names.sort();
    Some(names)
}

/// The reads of any one project.
fn project_reads(run: &Run) -> usize {
    run.requests
        .iter()
        .filter(|request| {
            request.method == "GET"
                && request
                    .path
                    .strip_prefix("/v0/projects/")
                    .is_some_and(|project| !project.contains('/'))
        })
        .count()
}
