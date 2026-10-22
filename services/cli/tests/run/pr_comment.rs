//! One pull request comment per Project, with a section per job.

use bencher_comment::{SectionCheck, group_tag};

use super::{
    ATTEMPTS, CHECK_LIST, CHECK_RUNS, COMMENTS, CONSOLE_PATH, CheckList, CheckRuns, GITHUB_TOKEN,
    GitHubApi, HEAD_SHA, JOB, JobRead, NEW_CHECK, PROJECT, PULL, Run, Scene, StandIn, UUID,
    VersionRead, bencher_run_in_scene, comment_tag, json_report, pull_request,
};

const OTHER_SHA: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678";
const REPORT_B: &str = "b0b0b0b0-b0b0-4b0b-8b0b-b0b0b0b0b0b0";
const REPORT_C: &str = "c0c0c0c0-c0c0-4c0c-8c0c-c0c0c0c0c0c0";
const REPORT_D: &str = "d0d0d0d0-d0d0-4d0d-8d0d-d0d0d0d0d0d0";
const OWN_NAME: &str = "Bencher Report (Project)";
const OWN_TAG_ID: &str = "main/base/magic";

/// An attach on a pull request event: it starts its check, then completes it with the report.
fn attach_run(scene: Scene, args: &[&str]) -> Run {
    attach_run_with(CheckRuns::Create, scene, args)
}

fn attach_run_with(check_runs: CheckRuns, scene: Scene, args: &[&str]) -> Run {
    bencher_run_in_scene(
        StandIn {
            job_read: JobRead::Callback("delivered"),
            version: VersionRead::Current,
            check_runs,
        },
        scene,
        Some(&pull_request(GitHubApi::StandIn)),
        &[],
        &[
            &ATTEMPTS[..],
            &PROJECT,
            &["--github-actions", GITHUB_TOKEN, "--job", JOB],
            args,
        ]
        .concat(),
    )
}

/// A check run as GitHub lists it.
fn listed(check: &SectionCheck) -> serde_json::Value {
    serde_json::json!({
        "id": check.id,
        "head_sha": HEAD_SHA,
        "name": check.name,
        "status": if check.conclusion.is_some() { "completed" } else { "in_progress" },
        "conclusion": check.conclusion,
        "external_id": check.external_id,
    })
}

fn section(id: u64, name: &str, conclusion: Option<&str>, external_id: &str) -> SectionCheck {
    SectionCheck {
        id,
        name: name.to_owned(),
        conclusion: conclusion.map(ToOwned::to_owned),
        external_id: external_id.to_owned(),
    }
}

/// The writer's own completed check.
fn own(external_id: &str) -> SectionCheck {
    section(NEW_CHECK, OWN_NAME, Some("success"), external_id)
}

fn own_tag() -> String {
    format!("bencher:{UUID}:{UUID}:t")
}

fn external_id(report: &str, kind: &str) -> String {
    format!("bencher:{UUID}:{report}:{kind}")
}

/// The section of another job, `w`, complete.
fn w() -> SectionCheck {
    section(
        4000,
        "Bencher Report (w)",
        Some("success"),
        &external_id(REPORT_B, "c"),
    )
}

#[expect(clippy::unwrap_used, reason = "test helper")]
fn the_group_tag(head_sha: &str) -> String {
    group_tag(&"project".parse().unwrap(), &head_sha.parse().unwrap())
}

/// A group comment body for the head SHA that holds `checks` as its records.
fn group_body(checks: &[SectionCheck]) -> String {
    group_body_at(HEAD_SHA, checks)
}

fn group_body_at(head_sha: &str, checks: &[SectionCheck]) -> String {
    let records = checks.iter().map(SectionCheck::html).collect::<String>();
    format!(
        "<h2>Bencher Report</h2>{records}{}",
        the_group_tag(head_sha)
    )
}

/// The report of another job on `testbed`.
#[expect(clippy::indexing_slicing, reason = "test helper")]
fn report(uuid: &str, testbed: &str) -> serde_json::Value {
    let mut report = json_report();
    report["uuid"] = uuid.into();
    report["testbed"]["name"] = testbed.into();
    report["testbed"]["slug"] = testbed.into();
    report
}

/// The bodies the run wrote to pull request comments: every create and every update, in order.
#[expect(clippy::indexing_slicing, clippy::unwrap_used, reason = "test helper")]
fn comment_writes(run: &Run) -> Vec<(String, String)> {
    run.requests
        .iter()
        .filter(|request| {
            request.method == "POST"
                && (request.path == COMMENTS
                    || request
                        .path
                        .starts_with("/repos/owner/repo/issues/comments/"))
        })
        .map(|request| {
            let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
            (
                request.path.clone(),
                body["body"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn deletes(run: &Run) -> Vec<&str> {
    run.requests
        .iter()
        .filter(|request| request.method == "DELETE")
        .map(|request| request.path.as_str())
        .collect()
}

/// The only comment left on the pull request.
#[expect(clippy::panic, reason = "test helper")]
fn only_comment(run: &Run) -> (u64, &str) {
    let [(id, body)] = run.scene.comments.as_slice() else {
        panic!("{:?}", run.scene.comments);
    };
    (*id, body)
}

fn ids(body: &str) -> Vec<u64> {
    SectionCheck::parse_all(body)
        .into_iter()
        .map(|check| check.id)
        .collect()
}

// Kills a single section that drifts from today's comment, a record that is not the completed
// check, a body that keeps today's tag, and a start or completion without the `external_id`.
#[test]
fn one_section_is_today_s_body_with_the_record_and_the_group_tag() {
    let run = attach_run(Scene::default(), &[]);
    run.assert_success();

    let (_, body) = only_comment(&run);
    let summary = run.step_summary.clone().unwrap();
    let today = summary.strip_suffix(&comment_tag(OWN_TAG_ID)).unwrap();
    assert_eq!(
        body,
        format!(
            "{today}{}{}",
            own(&own_tag()).html(),
            the_group_tag(HEAD_SHA)
        )
    );

    assert_eq!(
        run.body("POST", CHECK_RUNS)["external_id"],
        format!("bencher:{UUID}::t")
    );
    let complete = format!("{CHECK_RUNS}/{NEW_CHECK}");
    assert_eq!(run.body("PATCH", &complete)["external_id"], own_tag());
    let lists = run.requests_to("GET", CHECK_LIST);
    assert!(!lists.is_empty());
    for list in lists {
        assert!(
            list.query.contains("filter=all") && list.query.contains("per_page=100"),
            "{}",
            list.query
        );
    }
}

// Kills a loop that writes once, or that does not list the checks again after it writes.
#[test]
fn renders_that_cross_heal_on_the_next_list() {
    let other = section(
        4000,
        "Bencher Report (other)",
        Some("success"),
        &external_id(REPORT_B, "c"),
    );
    let run = attach_run(
        Scene {
            check_lists: vec![
                CheckList::Runs(Vec::new()),
                CheckList::Runs(vec![listed(&other)]),
            ],
            reports: vec![report(REPORT_B, "other")],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();

    let writes = comment_writes(&run);
    assert_eq!(writes.len(), 2, "{writes:?}");
    assert_eq!(ids(&writes[0].1), [NEW_CHECK]);
    let (id, body) = only_comment(&run);
    assert_eq!(
        writes[1].0,
        format!("/repos/owner/repo/issues/comments/{id}")
    );
    assert_eq!(writes[1].1, body);
    assert_eq!(ids(body), [NEW_CHECK, 4000]);
    assert!(body.contains(">2 (other)</a></td>"), "{body}");
    assert!(
        body.contains("/reports/b0b0b0b0-b0b0-4b0b-8b0b-b0b0b0b0b0b0"),
        "{body}"
    );
}

// Kills a merge that lets a late in-progress list replace a completed record, that drops a
// section the list omits, or a target that prefers a lower comment with today's tag to the
// group comment, or that keeps that comment.
#[test]
fn a_late_list_moves_no_section_back() {
    let b_done = section(
        4000,
        "Bencher Report (b)",
        Some("success"),
        &external_id(REPORT_B, "c"),
    );
    let b_running = section(
        4000,
        "Bencher Report (b)",
        None,
        &format!("bencher:{UUID}::c"),
    );
    let c_done = section(
        4100,
        "Bencher Report (c)",
        Some("failure"),
        &external_id(REPORT_C, "c"),
    );
    let run = attach_run(
        Scene {
            comments: vec![
                (2, format!("<p>old</p>{}", comment_tag(OWN_TAG_ID))),
                (3, group_body(&[b_done.clone(), c_done.clone()])),
            ],
            check_lists: vec![CheckList::Runs(vec![listed(&b_running)])],
            reports: vec![report(REPORT_B, "testbed-b"), report(REPORT_C, "testbed-c")],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();

    let writes = comment_writes(&run);
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert_eq!(writes[0].0, "/repos/owner/repo/issues/comments/3");
    assert_eq!(deletes(&run), ["/repos/owner/repo/issues/comments/2"]);
    let (_, body) = only_comment(&run);
    assert_eq!(
        SectionCheck::parse_all(body),
        [own(&own_tag()), b_done, c_done],
        "{body}"
    );
    assert!(!body.contains("Pending"), "{body}");
}

// Kills a merge that reads the records of a comment written for another head SHA.
#[test]
fn a_comment_from_an_older_push_is_replaced_whole() {
    let old = section(
        4000,
        "Bencher Report (old)",
        Some("success"),
        &external_id(REPORT_B, "c"),
    );
    let run = attach_run(
        Scene {
            comments: vec![(3, group_body_at(OTHER_SHA, &[old]))],
            reports: vec![report(REPORT_B, "base")],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    let (id, body) = only_comment(&run);
    assert_eq!(id, 3);
    assert_eq!(ids(body), [NEW_CHECK], "{body}");
    assert!(body.ends_with(&the_group_tag(HEAD_SHA)), "{body}");
}

// Kills a pending section whose report renders as a finished job without benchmarks.
#[test]
fn a_pending_section_links_its_report() {
    let detached = section(
        4000,
        "Bencher Report (detached)",
        None,
        &external_id(REPORT_B, "c"),
    );
    let run = attach_run(
        Scene {
            check_lists: vec![CheckList::Runs(vec![listed(&detached)])],
            reports: vec![report(REPORT_B, "base")],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    let (_, body) = only_comment(&run);
    assert!(
        body.contains(
            "<tr><td><a href=\"http://localhost:3000/console/projects/project/reports/b0b0b0b0-b0b0-4b0b-8b0b-b0b0b0b0b0b0"
        ),
        "{body}"
    );
    assert!(
        body.contains(
            ">2 (detached)</a></td><td><a href=\"http://localhost:3000/console/projects/project/testbeds/base"
        ),
        "{body}"
    );
    assert!(
        body.contains(">Magic</a></td><td>⏳ Pending</td></tr>"),
        "{body}"
    );
    assert!(
        body.contains("<summary>Report 2 (detached): 0 benchmarks</summary><br /><blockquote><h3>Results pending until the remote job finishes.</h3></blockquote></details>"),
        "{body}"
    );
}

// Kills a job that keeps the comment it created over a lower one, deletes another job's comment,
// or writes over the lowest comment without its records.
#[test]
fn a_create_race_leaves_the_lowest_comment() {
    let x = section(
        4200,
        "Bencher Report (x)",
        None,
        &format!("bencher:{UUID}::c"),
    );
    let run = attach_run(
        Scene {
            next_comment: 20,
            race: Some((10, group_body(&[x]))),
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();

    assert_eq!(deletes(&run), ["/repos/owner/repo/issues/comments/20"]);
    let writes = comment_writes(&run);
    assert_eq!(
        writes.first().map(|(path, _)| path.as_str()),
        Some(COMMENTS)
    );
    assert!(
        writes
            .iter()
            .skip(1)
            .all(|(path, _)| path == "/repos/owner/repo/issues/comments/10"),
        "{writes:?}"
    );
    let (id, body) = only_comment(&run);
    assert_eq!(id, 10);
    assert_eq!(ids(body), [NEW_CHECK, 4200], "{body}");
}

// Kills a guard that is missing or that compares anything but the pull request's head.
#[test]
fn a_job_on_an_older_sha_writes_nothing() {
    let run = attach_run(
        Scene {
            pull_head: Some(OTHER_SHA),
            comments: vec![(3, comment_tag(OWN_TAG_ID))],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    assert_eq!(comment_writes(&run), []);
    assert_eq!(deletes(&run), Vec::<&str>::new());
    assert_eq!(run.scene.comments, [(3, comment_tag(OWN_TAG_ID))]);
    let complete = format!("{CHECK_RUNS}/{NEW_CHECK}");
    assert_eq!(run.body("PATCH", &complete)["conclusion"], "success");
}

// Kills a guard that refuses an issue number, which has no pull request to read.
#[test]
fn an_issue_number_skips_the_guard() {
    let run = attach_run(
        Scene {
            pull_head: None,
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    let (_, body) = only_comment(&run);
    assert!(body.ends_with(&the_group_tag(HEAD_SHA)), "{body}");
}

// Kills a takeover of any but the lowest of today's comments, a delete of a comment of another
// Project or of a person, and a 403 on a delete that fails the run.
#[test]
fn the_first_write_takes_over_today_s_comments() {
    let other_project = r#"<div id="bencher.dev/projects/other/id/main/base/magic"></div>"#;
    let run = attach_run(
        Scene {
            comments: vec![
                (3, format!("<p>old</p>{}", comment_tag(OWN_TAG_ID))),
                (5, format!("<p>old</p>{}", comment_tag("suite"))),
                (6, format!("<p>old</p>{}", comment_tag("pinned"))),
                (8, "LGTM".to_owned()),
                (9, other_project.to_owned()),
            ],
            undeletable: vec![6],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    assert!(!run.stderr().contains("Warning"), "{}", run.stderr());

    assert_eq!(
        deletes(&run),
        [
            "/repos/owner/repo/issues/comments/5",
            "/repos/owner/repo/issues/comments/6"
        ]
    );
    let comments = &run.scene.comments;
    assert_eq!(
        comments.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        [3, 6, 8, 9]
    );
    assert!(
        comments[0].1.ends_with(&the_group_tag(HEAD_SHA)),
        "{comments:?}"
    );
    assert_eq!(comments[2].1, "LGTM");
    assert_eq!(comments[3].1, other_project);
}

// Kills a fallback that is missing, that writes a group comment, or that drops today's tag.
#[test]
fn a_forbidden_check_list_posts_today_s_comment() {
    let run = attach_run(
        Scene {
            check_lists: vec![CheckList::Forbidden],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    let (_, body) = only_comment(&run);
    assert!(body.ends_with(&comment_tag(OWN_TAG_ID)), "{body}");
    assert!(SectionCheck::parse_all(body).is_empty(), "{body}");
}

// Kills sections grouped by check name, which two jobs without `--ci-id` share.
#[test]
fn two_testbeds_without_a_ci_id_are_two_sections() {
    let other = section(5000, OWN_NAME, Some("success"), &external_id(REPORT_B, "t"));
    let run = attach_run(
        Scene {
            check_lists: vec![CheckList::Runs(vec![
                listed(&own(&own_tag())),
                listed(&other),
            ])],
            reports: vec![report(REPORT_B, "other")],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    let (_, body) = only_comment(&run);
    assert_eq!(ids(body), [NEW_CHECK, 5000], "{body}");
    for testbed in [">base</a></td>", ">other</a></td>"] {
        assert!(body.contains(testbed), "{testbed}: {body}");
    }
    // Without `--ci-id`, a row's label is its number alone.
    for label in [">1</a></td>", ">2</a></td>"] {
        assert!(body.contains(label), "{label}: {body}");
    }
}

// Kills a key with no highest check run rule, so a rerun's older check is a second section.
#[test]
fn a_rerun_s_newer_check_run_wins() {
    let older = section(
        100,
        "Bencher Report (suite)",
        Some("failure"),
        &external_id(REPORT_B, "c"),
    );
    let run = attach_run(
        Scene {
            check_lists: vec![CheckList::Runs(vec![listed(&older)])],
            reports: vec![report(REPORT_B, "base")],
            ..Scene::default()
        },
        &["--ci-id", "suite"],
    );
    run.assert_success();
    let (_, body) = only_comment(&run);
    assert_eq!(ids(body), [NEW_CHECK], "{body}");
    assert!(!body.contains("<th>Report</th>"), "{body}");
}

// Kills a writer that renders its section over a newer check run of the same key.
#[test]
fn an_older_writer_for_a_key_writes_nothing() {
    let newer = section(
        6000,
        "Bencher Report (suite)",
        None,
        &format!("bencher:{UUID}::c"),
    );
    let run = attach_run(
        Scene {
            check_lists: vec![CheckList::Runs(vec![listed(&newer)])],
            ..Scene::default()
        },
        &["--ci-id", "suite"],
    );
    run.assert_success();
    assert_eq!(comment_writes(&run), []);
    assert_eq!(run.scene.comments, []);
}

// Kills a start that leaves out the kind of a job with `--ci-id`.
#[test]
fn the_start_names_the_project_and_the_kind() {
    let run = attach_run(Scene::default(), &["--ci-id", "suite"]);
    run.assert_success();
    assert_eq!(
        run.body("POST", CHECK_RUNS)["external_id"],
        format!("bencher:{UUID}::c")
    );
    let complete = format!("{CHECK_RUNS}/{NEW_CHECK}");
    assert_eq!(
        run.body("PATCH", &complete)["external_id"],
        external_id(UUID, "c")
    );
    assert_eq!(
        run.github().get(..3),
        Some(
            &[
                ("POST", CHECK_RUNS),
                ("PATCH", complete.as_str()),
                ("GET", PULL)
            ][..]
        )
    );
}

// Kills a loop that stops while the comment lacks the job's own section, which another writer
// dropped between this job's write and its read back.
#[test]
fn overwritten_without_its_own_section_writes_again() {
    let run = attach_run(
        Scene {
            check_lists: vec![CheckList::Runs(vec![listed(&w())])],
            reports: vec![report(REPORT_B, "w")],
            overwrite_on_read: Some(group_body(&[w()])),
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    let (_, body) = only_comment(&run);
    assert_eq!(ids(body), [NEW_CHECK, 4000], "{body}");
}

// Kills a loop that stops while the comment carries an older head SHA's render,
// which a job of that SHA wrote after this job's guard.
#[test]
fn overwritten_by_an_older_sha_writes_again() {
    let old = section(
        4000,
        "Bencher Report (old)",
        Some("success"),
        &external_id(REPORT_B, "c"),
    );
    let run = attach_run(
        Scene {
            overwrite_on_read: Some(group_body_at(OTHER_SHA, &[old])),
            reports: vec![report(REPORT_B, "old")],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    let (_, body) = only_comment(&run);
    assert!(body.ends_with(&the_group_tag(HEAD_SHA)), "{body}");
    assert_eq!(ids(body), [NEW_CHECK], "{body}");
}

// Kills a failed delete of an old comment that fails the run after the comment landed.
#[test]
fn a_failed_delete_after_the_write_is_a_warning() {
    let run = attach_run(
        Scene {
            comments: vec![
                (3, format!("<p>old</p>{}", comment_tag(OWN_TAG_ID))),
                (5, format!("<p>old</p>{}", comment_tag("suite"))),
            ],
            delete_status: Some("422 Unprocessable Entity"),
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    assert!(run.stderr().contains("Warning"), "{}", run.stderr());
    let (_, body) = &run.scene.comments[0];
    assert_eq!(ids(body), [NEW_CHECK], "{body}");
}

// Kills a pull request read without permission that fails the run, rather than skip the guard.
#[test]
fn a_forbidden_pull_request_read_skips_the_guard() {
    let run = attach_run(
        Scene {
            pull_status: Some("403 Forbidden"),
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    assert!(run.stderr().contains("Warning"), "{}", run.stderr());
    let (_, body) = only_comment(&run);
    assert_eq!(ids(body), [NEW_CHECK], "{body}");
}

// Kills a rewrite that a rate limit fails, which fails the run after the first write landed.
#[test]
fn a_rate_limited_rewrite_is_a_warning() {
    let run = attach_run(
        Scene {
            check_lists: vec![
                CheckList::Runs(Vec::new()),
                CheckList::Runs(vec![listed(&w())]),
            ],
            reports: vec![report(REPORT_B, "w")],
            writes_before_limit: Some(1),
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    assert!(run.stderr().contains("Warning"), "{}", run.stderr());
    let (_, body) = only_comment(&run);
    assert_eq!(ids(body), [NEW_CHECK], "{body}");
}

// Kills a failed report fetch that is cached, or that counts as settled, so one transient
// failure drops a finished `t` section for the whole write.
#[test]
fn a_failed_report_fetch_is_tried_again() {
    let other = section(5000, OWN_NAME, Some("success"), &external_id(REPORT_C, "t"));
    let run = attach_run(
        Scene {
            check_lists: vec![CheckList::Runs(vec![listed(&other)])],
            reports: vec![report(REPORT_C, "other")],
            report_failures: vec![REPORT_C],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    let (_, body) = only_comment(&run);
    assert_eq!(ids(body), [NEW_CHECK, 5000], "{body}");
}

// Kills a read back whose body does not feed the next write, and a console URL read per report.
#[test]
fn read_back_records_survive_a_late_list() {
    let own = own(&own_tag());
    let x = section(
        4100,
        "Bencher Report (x)",
        Some("success"),
        &external_id(REPORT_C, "c"),
    );
    let y = section(
        4200,
        "Bencher Report (y)",
        Some("success"),
        &external_id(REPORT_D, "c"),
    );
    let run = attach_run(
        Scene {
            check_lists: vec![
                CheckList::Runs(vec![listed(&w())]),
                CheckList::Runs(vec![listed(&w()), listed(&y)]),
            ],
            reports: vec![
                report(REPORT_B, "w"),
                report(REPORT_C, "x"),
                report(REPORT_D, "y"),
            ],
            overwrite_on_read: Some(group_body(&[w(), x, own])),
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    let (_, body) = only_comment(&run);
    assert_eq!(ids(body), [NEW_CHECK, 4000, 4100, 4200], "{body}");
    // Once for this job's own report, which the other reports share.
    assert_eq!(run.requests_to("GET", CONSOLE_PATH).len(), 1);
}

// Kills a group write without the `--ci-only-thresholds` gate.
#[test]
fn only_thresholds_writes_nothing_without_a_threshold() {
    let run = attach_run(Scene::default(), &["--ci-only-thresholds"]);
    run.assert_success();
    assert_eq!(comment_writes(&run), []);
}

// Kills a group write without the `--ci-only-on-alert` gate, or one that also holds back an update.
#[test]
fn only_on_alert_creates_nothing_without_an_alert() {
    let run = attach_run(Scene::default(), &["--ci-only-on-alert"]);
    run.assert_success();
    assert_eq!(comment_writes(&run), []);

    let run = attach_run(
        Scene {
            comments: vec![(3, group_body(&[w()]))],
            reports: vec![report(REPORT_B, "w")],
            ..Scene::default()
        },
        &["--ci-only-on-alert"],
    );
    run.assert_success();
    let (_, body) = only_comment(&run);
    assert_eq!(ids(body), [NEW_CHECK, 4000], "{body}");
}

// Kills a job that posts nothing when it could neither start nor complete its check.
#[test]
fn a_failed_check_posts_today_s_comment() {
    let run = attach_run_with(CheckRuns::Unprocessable, Scene::default(), &[]);
    run.assert_success();
    let (_, body) = only_comment(&run);
    assert!(body.ends_with(&comment_tag(OWN_TAG_ID)), "{body}");
}

// Kills an update that fails on a target another job deleted, rather than write it again.
#[test]
fn a_vanished_target_is_written_again() {
    let run = attach_run(
        Scene {
            comments: vec![(3, group_body(&[w()]))],
            check_lists: vec![CheckList::Runs(vec![listed(&w())])],
            reports: vec![report(REPORT_B, "w")],
            vanish_on_update: vec![3],
            next_comment: 20,
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    let (id, body) = only_comment(&run);
    assert_eq!(id, 20);
    assert_eq!(ids(body), [NEW_CHECK, 4000], "{body}");
}

// Kills a read back that stops at a gone target, rather than write to the one that replaced it.
#[test]
fn a_target_gone_at_read_back_moves_to_the_new_one() {
    let run = attach_run(
        Scene {
            comments: vec![(3, format!("<p>old</p>{}", comment_tag(OWN_TAG_ID)))],
            check_lists: vec![CheckList::Runs(vec![listed(&w())])],
            reports: vec![report(REPORT_B, "w")],
            replace_on_read: Some((10, group_body(&[w()]))),
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    let (id, body) = only_comment(&run);
    assert_eq!(id, 10);
    assert_eq!(ids(body), [NEW_CHECK, 4000], "{body}");
}

// Kills a read back that writes over a newer head SHA's comment without guarding again.
#[test]
fn a_newer_sha_at_read_back_stops_the_job() {
    let run = attach_run(
        Scene {
            check_lists: vec![
                CheckList::Runs(Vec::new()),
                CheckList::Runs(vec![listed(&w())]),
            ],
            reports: vec![report(REPORT_B, "w")],
            overwrite_on_read: Some(group_body_at(OTHER_SHA, &[])),
            moved_head: Some((1, OTHER_SHA)),
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    assert_eq!(comment_writes(&run).len(), 1, "{:?}", comment_writes(&run));
    let (_, body) = only_comment(&run);
    assert!(body.ends_with(&the_group_tag(OTHER_SHA)), "{body}");
}

// Kills a target, a record source, or an old comment taken from a comment that another author
// wrote, however it ends.
#[test]
fn a_person_s_comment_is_not_trusted() {
    let planted = group_body(&[w()]);
    let today = format!("<p>old</p>{}", comment_tag(OWN_TAG_ID));
    let run = attach_run(
        Scene {
            comments: vec![(3, planted.clone()), (4, today.clone())],
            by_person: vec![3, 4],
            reports: vec![report(REPORT_B, "w")],
            next_comment: 20,
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    assert_eq!(deletes(&run), Vec::<&str>::new());
    let comments = &run.scene.comments;
    assert_eq!(
        comments.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        [3, 4, 20]
    );
    assert_eq!(comments[0].1, planted);
    assert_eq!(comments[1].1, today);
    assert_eq!(ids(&comments[2].1), [NEW_CHECK], "{comments:?}");
}

// Kills a check list that reads only its first page.
#[test]
fn the_check_list_reads_every_page() {
    let mut runs = (1..=100)
        .map(|id| listed(&section(id, "lint", Some("success"), "")))
        .collect::<Vec<_>>();
    runs.push(listed(&w()));
    let run = attach_run(
        Scene {
            check_lists: vec![CheckList::Runs(runs)],
            reports: vec![report(REPORT_B, "w")],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    let (_, body) = only_comment(&run);
    assert_eq!(ids(body), [NEW_CHECK, 4000], "{body}");
}

// Kills a delete that fails on a comment already gone, which stops the write early.
#[test]
fn a_delete_of_a_gone_comment_is_passed_over() {
    let run = attach_run(
        Scene {
            comments: vec![
                (3, format!("<p>old</p>{}", comment_tag(OWN_TAG_ID))),
                (5, format!("<p>old</p>{}", comment_tag("suite"))),
            ],
            delete_status: Some("404 Not Found"),
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    assert!(!run.stderr().contains("Warning"), "{}", run.stderr());
    assert_eq!(
        run.requests_to("GET", "/repos/owner/repo/issues/comments/3")
            .len(),
        1
    );
}

// Kills a completion that creates the check without the `external_id` when the start failed.
#[test]
fn the_fallback_create_names_the_report() {
    let run = attach_run_with(CheckRuns::StartFails, Scene::default(), &[]);
    run.assert_success();
    let creates = run.bodies("POST", CHECK_RUNS);
    assert_eq!(creates.len(), 2, "{creates:?}");
    assert_eq!(creates[1]["external_id"], own_tag());
    let (_, body) = only_comment(&run);
    assert_eq!(ids(body), [NEW_CHECK], "{body}");
}

// Kills today's comment without the `--ci-only-thresholds` gate.
#[test]
fn only_thresholds_holds_today_s_comment_too() {
    let run = attach_run(
        Scene {
            check_lists: vec![CheckList::Forbidden],
            ..Scene::default()
        },
        &["--ci-only-thresholds"],
    );
    run.assert_success();
    assert_eq!(comment_writes(&run), []);
}

// Kills a job that writes a comment that already holds every section as the job would render it,
// or that leaves today's comment beside it.
#[test]
fn a_comment_that_holds_the_sections_is_not_written() {
    let held = group_body(&[own(&own_tag()), w()]);
    let run = attach_run(
        Scene {
            comments: vec![
                (3, held.clone()),
                (5, format!("<p>old</p>{}", comment_tag("suite"))),
            ],
            check_lists: vec![CheckList::Runs(vec![listed(&w())])],
            reports: vec![report(REPORT_B, "w")],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    assert_eq!(comment_writes(&run), []);
    assert_eq!(run.scene.comments, [(3, held)]);
}

// Kills a job that stops at a comment holding every section it could render,
// while the report of another section could not be fetched.
#[test]
fn a_held_comment_waits_for_a_failed_report() {
    let other = section(5000, OWN_NAME, Some("success"), &external_id(REPORT_C, "t"));
    let run = attach_run(
        Scene {
            comments: vec![(3, group_body(&[own(&own_tag())]))],
            check_lists: vec![CheckList::Runs(vec![listed(&other)])],
            reports: vec![report(REPORT_C, "other")],
            report_failures: vec![REPORT_C],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    let (_, body) = only_comment(&run);
    assert_eq!(ids(body), [NEW_CHECK, 5000], "{body}");
}

// Kills a fetch of another section's report that retries, which can hold the write for minutes.
#[test]
fn another_section_s_report_is_fetched_once_a_round() {
    let other = section(5000, OWN_NAME, Some("success"), &external_id(REPORT_C, "t"));
    let run = bencher_run_in_scene(
        StandIn {
            job_read: JobRead::Callback("delivered"),
            version: VersionRead::Current,
            check_runs: CheckRuns::Create,
        },
        Scene {
            check_lists: vec![CheckList::Runs(vec![listed(&other)])],
            reports: vec![report(REPORT_C, "other")],
            report_failures: vec![REPORT_C],
            ..Scene::default()
        },
        Some(&pull_request(GitHubApi::StandIn)),
        &[],
        &[
            &["--attempts", "2"][..],
            &PROJECT,
            &["--github-actions", GITHUB_TOKEN, "--job", JOB],
        ]
        .concat(),
    );
    run.assert_success();
    let writes = comment_writes(&run);
    assert_eq!(ids(&writes[0].1), [NEW_CHECK], "{writes:?}");
    let (_, body) = only_comment(&run);
    assert_eq!(ids(body), [NEW_CHECK, 5000], "{body}");
}

// Kills a comment list after a create that fails the run, although the comment landed.
#[test]
fn a_failed_list_after_the_create_is_a_warning() {
    let run = attach_run(
        Scene {
            lists_before_failure: Some(1),
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    assert!(run.stderr().contains("Warning"), "{}", run.stderr());
    let (_, body) = only_comment(&run);
    assert_eq!(ids(body), [NEW_CHECK], "{body}");
}

// Kills a held target that counts only the check run IDs, so a section in an older state stays.
#[test]
fn held_ids_with_an_older_state_are_written() {
    let w_running = section(
        4000,
        "Bencher Report (w)",
        None,
        &format!("bencher:{UUID}::c"),
    );
    let run = attach_run(
        Scene {
            comments: vec![(3, group_body(&[own(&own_tag()), w_running]))],
            check_lists: vec![CheckList::Runs(vec![listed(&w())])],
            reports: vec![report(REPORT_B, "w")],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    let (_, body) = only_comment(&run);
    assert!(SectionCheck::parse_all(body).contains(&w()), "{body}");
}

// Kills a job that settles while its own render lacks a `c` section's report that it can fetch now,
// or that keeps counting a report as missing after it renders it.
#[test]
fn a_c_report_fetched_late_is_rendered() {
    let run = attach_run(
        Scene {
            check_lists: vec![CheckList::Runs(vec![listed(&w())])],
            reports: vec![report(REPORT_B, "w")],
            report_failures: vec![REPORT_B],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    assert_eq!(comment_writes(&run).len(), 2, "{:?}", comment_writes(&run));
    let (_, body) = only_comment(&run);
    assert!(
        body.contains("/reports/b0b0b0b0-b0b0-4b0b-8b0b-b0b0b0b0b0b0"),
        "{body}"
    );
}

// Kills a group write for a check whose app GitHub does not name, or names without a slug,
// which leaves the job no author to trust.
#[test]
fn a_check_without_an_app_posts_today_s_comment() {
    for app in [serde_json::Value::Null, serde_json::json!({ "id": 15368 })] {
        let group = group_body(&[w()]);
        let run = attach_run(
            Scene {
                comments: vec![(3, group.clone())],
                check_app: Some(app.clone()),
                next_comment: 20,
                ..Scene::default()
            },
            &[],
        );
        run.assert_success();
        let comments = &run.scene.comments;
        assert_eq!(comments.len(), 2, "{app}: {comments:?}");
        assert_eq!(comments[0], (3, group), "{app}");
        assert!(
            comments[1].1.ends_with(&comment_tag(OWN_TAG_ID)),
            "{app}: {comments:?}"
        );
    }
}

// Kills a held target that does not count as posted, so a failed list after it fails the run.
#[test]
fn a_failed_list_after_a_held_target_is_a_warning() {
    let other = section(5000, OWN_NAME, Some("success"), &external_id(REPORT_C, "t"));
    let run = attach_run(
        Scene {
            comments: vec![(3, group_body(&[own(&own_tag())]))],
            check_lists: vec![CheckList::Runs(vec![listed(&other)]), CheckList::Forbidden],
            reports: vec![report(REPORT_C, "other")],
            report_failures: vec![REPORT_C],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    assert!(run.stderr().contains("Warning"), "{}", run.stderr());
    assert_eq!(comment_writes(&run), []);
}

// Kills a held target whose tag is for another head SHA, however exactly its records match.
#[test]
fn a_comment_for_another_sha_is_written_though_its_records_match() {
    let run = attach_run(
        Scene {
            comments: vec![(3, group_body_at(OTHER_SHA, &[own(&own_tag())]))],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    let (_, body) = only_comment(&run);
    assert!(body.ends_with(&the_group_tag(HEAD_SHA)), "{body}");
}

// Kills a report fetched in one state that stands for the same report in the other: a pending
// section's report must not settle its completed section, nor the completed one its pending one.
#[test]
fn a_report_in_one_state_does_not_stand_for_the_other() {
    let x_pending = section(
        4100,
        "Bencher Report (x)",
        None,
        &external_id(REPORT_B, "c"),
    );
    let x_done = section(
        4100,
        "Bencher Report (x)",
        Some("success"),
        &external_id(REPORT_B, "c"),
    );
    let run = attach_run(
        Scene {
            check_lists: vec![
                CheckList::Runs(vec![listed(&x_pending)]),
                CheckList::Runs(vec![listed(&x_done)]),
            ],
            reports: vec![report(REPORT_B, "testbed-x")],
            report_ok_then_fail: Some((REPORT_B, 1)),
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    assert_eq!(comment_writes(&run).len(), 2, "{:?}", comment_writes(&run));

    let run = attach_run(
        Scene {
            check_lists: vec![CheckList::Runs(vec![listed(&x_pending)])],
            reports: vec![report(REPORT_B, "testbed-x")],
            report_failures: vec![REPORT_B],
            ..Scene::default()
        },
        &[],
    );
    run.assert_success();
    let (_, body) = only_comment(&run);
    assert!(
        body.contains(
            ">2 (x)</a></td><td><a href=\"http://localhost:3000/console/projects/project/testbeds/testbed-x"
        ),
        "{body}"
    );
}

// Kills an own record built from the request rather than from GitHub's answer to it.
#[test]
fn the_own_record_is_github_s_check() {
    let run = attach_run(
        Scene {
            completed_name: Some("Bencher Report (normalized)"),
            ..Scene::default()
        },
        &["--ci-id", "suite"],
    );
    run.assert_success();
    let (_, body) = only_comment(&run);
    let records = SectionCheck::parse_all(body);
    assert_eq!(records.len(), 1, "{body}");
    assert_eq!(records[0].name, "Bencher Report (normalized)", "{body}");
}
