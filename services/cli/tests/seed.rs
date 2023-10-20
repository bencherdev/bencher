#![cfg(feature = "seed")]

use std::process::Command;

use assert_cmd::prelude::*;
use once_cell::sync::Lazy;
use pretty_assertions::assert_eq;

const BENCHER_CMD: &str = "bencher";

const HOST_ARG: &str = "--host";
const LOCALHOST: &str = "http://localhost:61016";

const TOKEN_ARG: &str = "--token";
const PROJECT_ARG: &str = "--project";
const PROJECT_SLUG: &str = "the-computer";
const BRANCH_ARG: &str = "--branch";
const BRANCH_SLUG: &str = "master";
const TESTBED_ARG: &str = "--testbed";
const TESTBED_SLUG: &str = "base";
const METRIC_KIND_ARG: &str = "--metric-kind";
const METRIC_KIND_SLUG: &str = "screams";

// Valid until 2027-09-05T19:03:59Z
const TEST_BENCHER_API_TOKEN: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjoxODIwMTcxMDM5LCJpYXQiOjE2NjIzODY0MDksImlzcyI6ImJlbmNoZXIuZGV2Iiwic3ViIjoibXVyaWVsLmJhZ2dlQG5vd2hlcmUuY29tIn0.sfAJmF9qIl_QRNnh8uLYuODHnxufXt_3m7skcNp1kMs";

pub const BENCHER_API_URL: &str = "BENCHER_API_URL";
pub static HOST_URL: Lazy<String> =
    Lazy::new(|| std::env::var(BENCHER_API_URL).unwrap_or_else(|_| LOCALHOST.to_owned()));

// cargo test --features seed --test seed
#[test]
#[allow(clippy::too_many_lines, clippy::unwrap_used)]
fn test_cli_seed() -> Result<(), Box<dyn std::error::Error>> {
    // Signup Eustace Bagge first so he is admin
    // cargo run -- auth signup --host http://localhost:61016 --name "Eustace Bagge" eustace.bagge@nowhere.com
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "auth",
        "signup",
        HOST_ARG,
        &HOST_URL,
        "--name",
        "Eustace Bagge",
        "eustace.bagge@nowhere.com",
    ]);
    let assert = cmd.assert().success();
    let _json: bencher_json::JsonEmpty =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();

    // cargo run -- auth signup --host http://localhost:61016 --name "Muriel Bagge" muriel.bagge@nowhere.com
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "auth",
        "signup",
        HOST_ARG,
        &HOST_URL,
        "--name",
        "Muriel Bagge",
        "muriel.bagge@nowhere.com",
    ]);
    let assert = cmd.assert().success();
    let _json: bencher_json::JsonEmpty =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();

    // cargo run -- auth login --host http://localhost:61016 muriel.bagge@nowhere.com
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "auth",
        "login",
        HOST_ARG,
        &HOST_URL,
        "muriel.bagge@nowhere.com",
    ]);
    let assert = cmd.assert().success();
    let _json: bencher_json::JsonEmpty =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();

    // cargo run -- auth confirm --host http://localhost:61016 [AUTH_TOKEN]
    // cargo run -- token ls --host http://localhost:61016 --user muriel-bagge
    // cargo run -- token create --host http://localhost:61016 --user muriel-bagge --ttl 157784630 TEST_BENCHER_API_TOKEN

    // cargo run -- org ls --host http://localhost:61016
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "org",
        "ls",
        HOST_ARG,
        &HOST_URL,
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
    ]);
    let assert = cmd.assert().success();
    let organizations: bencher_json::JsonOrganizations =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(organizations.0.len(), 1);

    // cargo run -- org view --host http://localhost:61016 muriel-bagge
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "org",
        "view",
        HOST_ARG,
        &HOST_URL,
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
        "muriel-bagge",
    ]);
    let assert = cmd.assert().success();
    let _json: bencher_json::JsonOrganization =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();

    // cargo run -- project ls --host http://localhost:61016 --public
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args(["project", "ls", HOST_ARG, &HOST_URL, "--public"]);
    let assert = cmd.assert().success();
    let projects: bencher_json::JsonProjects =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(projects.0.len(), 0);

    // cargo run -- project ls --host http://localhost:61016
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "project",
        "ls",
        HOST_ARG,
        &HOST_URL,
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
    ]);
    let assert = cmd.assert().success();
    let projects: bencher_json::JsonProjects =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(projects.0.len(), 0);

    // cargo run -- project ls --host http://localhost:61016 --org muriel-bagge
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "project",
        "ls",
        HOST_ARG,
        &HOST_URL,
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
        "--org",
        "muriel-bagge",
    ]);
    cmd.assert().success();
    let assert = cmd.assert().success();
    let projects: bencher_json::JsonProjects =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(projects.0.len(), 0);

    // cargo run -- project create --host http://localhost:61016 --org muriel-bagge "The Computer"
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "project",
        "create",
        HOST_ARG,
        &HOST_URL,
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
        "--org",
        "muriel-bagge",
        "--slug",
        PROJECT_SLUG,
        "The Computer",
    ]);
    let assert = cmd.assert().success();
    let _json: bencher_json::JsonProject =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();

    // cargo run -- project ls --host http://localhost:61016
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "project",
        "ls",
        HOST_ARG,
        &HOST_URL,
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
    ]);
    let assert = cmd.assert().success();
    let projects: bencher_json::JsonProjects =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(projects.0.len(), 1);

    // cargo run -- project view --host http://localhost:61016 the-computer
    // View project without token
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args(["project", "view", HOST_ARG, &HOST_URL, PROJECT_SLUG]);
    let assert = cmd.assert().success();
    let _json: bencher_json::JsonProject =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();

    // View project with token
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "project",
        "view",
        HOST_ARG,
        &HOST_URL,
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
        PROJECT_SLUG,
    ]);
    let assert = cmd.assert().success();
    let _json: bencher_json::JsonProject =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();

    // cargo run -- metric-kind ls --host http://localhost:61016 --project the-computer
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "metric-kind",
        "ls",
        HOST_ARG,
        &HOST_URL,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    let assert = cmd.assert().success();
    let metric_kinds: bencher_json::JsonMetricKinds =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(metric_kinds.0.len(), 2);

    // cargo run -- metric-kind create --host http://localhost:61016 --project the-computer --slug decibels-666 --units "decibels" screams-888
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "metric-kind",
        "create",
        HOST_ARG,
        &HOST_URL,
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
        PROJECT_ARG,
        PROJECT_SLUG,
        "--slug",
        METRIC_KIND_SLUG,
        "--units",
        "decibels",
        METRIC_KIND_SLUG,
    ]);
    let assert = cmd.assert().success();
    let _json: bencher_json::JsonMetricKind =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();

    // cargo run -- metric-kind ls --host http://localhost:61016 --project the-computer
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "metric-kind",
        "ls",
        HOST_ARG,
        &HOST_URL,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    let assert = cmd.assert().success();
    let metric_kinds: bencher_json::JsonMetricKinds =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(metric_kinds.0.len(), 3);

    // cargo run -- metric-kind view --host http://localhost:61016 --project the-computer screams-888
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "metric-kind",
        "view",
        HOST_ARG,
        &HOST_URL,
        PROJECT_ARG,
        PROJECT_SLUG,
        METRIC_KIND_SLUG,
    ]);
    let assert = cmd.assert().success();
    let _json: bencher_json::JsonMetricKind =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();

    // cargo run -- branch ls --host http://localhost:61016 --project the-computer
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "branch",
        "ls",
        HOST_ARG,
        &HOST_URL,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    let assert = cmd.assert().success();
    let branches: bencher_json::JsonBranches =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(branches.0.len(), 1);

    // cargo run -- branch create --host http://localhost:61016 --project the-computer master
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "branch",
        "create",
        HOST_ARG,
        &HOST_URL,
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
        PROJECT_ARG,
        PROJECT_SLUG,
        "--slug",
        BRANCH_SLUG,
        "master",
    ]);
    let assert = cmd.assert().success();
    let _json: bencher_json::JsonBranch =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();

    // cargo run -- branch ls --host http://localhost:61016 --project the-computer
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "branch",
        "ls",
        HOST_ARG,
        &HOST_URL,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    let assert = cmd.assert().success();
    let branches: bencher_json::JsonBranches =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(branches.0.len(), 2);

    // cargo run -- branch view --host http://localhost:61016 --project the-computer master
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "branch",
        "view",
        HOST_ARG,
        &HOST_URL,
        PROJECT_ARG,
        PROJECT_SLUG,
        BRANCH_SLUG,
    ]);
    let assert = cmd.assert().success();
    let _json: bencher_json::JsonBranch =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();

    // cargo run -- testbed ls --host http://localhost:61016 --project the-computer
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "testbed",
        "ls",
        HOST_ARG,
        &HOST_URL,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    let assert = cmd.assert().success();
    let testbeds: bencher_json::JsonTestbeds =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(testbeds.0.len(), 1);

    // cargo run -- testbed create --host http://localhost:61016 --project the-computer base
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "testbed",
        "create",
        HOST_ARG,
        &HOST_URL,
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
        PROJECT_ARG,
        PROJECT_SLUG,
        "--slug",
        TESTBED_SLUG,
        "base",
    ]);
    let assert = cmd.assert().success();
    let _json: bencher_json::JsonTestbed =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();

    // cargo run -- testbed ls --host http://localhost:61016 --project the-computer
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "testbed",
        "ls",
        HOST_ARG,
        &HOST_URL,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    let assert = cmd.assert().success();
    let testbeds: bencher_json::JsonTestbeds =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(testbeds.0.len(), 2);

    // cargo run -- testbed view --host http://localhost:61016 --project the-computer base
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "testbed",
        "view",
        HOST_ARG,
        &HOST_URL,
        PROJECT_ARG,
        PROJECT_SLUG,
        TESTBED_SLUG,
    ]);
    let assert = cmd.assert().success();
    let _json: bencher_json::JsonTestbed =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();

    // cargo run -- threshold ls --host http://localhost:61016 --project the-computer
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "threshold",
        "ls",
        HOST_ARG,
        &HOST_URL,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    let assert = cmd.assert().success();
    let testbeds: bencher_json::JsonThresholds =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(testbeds.0.len(), 2);

    // cargo run -- threshold create --host http://localhost:61016 --metric-kind latency --branch $BENCHER_BRANCH --testbed $BENCHER_TESTBED --test z
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "threshold",
        "create",
        HOST_ARG,
        &HOST_URL,
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
        PROJECT_ARG,
        PROJECT_SLUG,
        METRIC_KIND_ARG,
        METRIC_KIND_SLUG,
        BRANCH_ARG,
        BRANCH_SLUG,
        TESTBED_ARG,
        TESTBED_SLUG,
        "--test",
        "z",
    ]);
    let assert = cmd.assert().success();
    let _json: bencher_json::JsonThreshold =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();

    // cargo run -- threshold ls --host http://localhost:61016 --project the-computer
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "threshold",
        "ls",
        HOST_ARG,
        &HOST_URL,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    let assert = cmd.assert().success();
    let testbeds: bencher_json::JsonThresholds =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(testbeds.0.len(), 3);

    // cargo run -- member invite --host http://localhost:61016 --email courage@nowhere.com --org <ORG_UUID>
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "member",
        "invite",
        HOST_ARG,
        &HOST_URL,
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
        "--email",
        "courage@nowhere.com",
        "--org",
        "muriel-bagge",
        "--role",
        "leader",
    ]);
    let assert = cmd.assert().success();
    let _json: bencher_json::JsonEmpty =
        serde_json::from_slice(&assert.get_output().stdout).unwrap();

    // bencher run --iter 3 "bencher mock"
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    let bencher_mock = format!(
        r#"{bencher} mock"#,
        bencher = cmd.get_program().to_string_lossy()
    );
    cmd.args([
        "run",
        HOST_ARG,
        &HOST_URL,
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
        PROJECT_ARG,
        PROJECT_SLUG,
        BRANCH_ARG,
        BRANCH_SLUG,
        TESTBED_ARG,
        TESTBED_SLUG,
        "--iter",
        "3",
        &bencher_mock,
    ]);
    let _assert = cmd.assert().success();
    // println!("{}", String::from_utf8_lossy(&assert.get_output().stdout));

    Ok(())
}
