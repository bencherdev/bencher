#![cfg(feature = "seed")]

use std::{env, process::Command};

use assert_cmd::prelude::*;
use bencher_json::{JsonBranch, JsonOrganization, JsonProject, JsonTestbed};

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

const BENCHER_API_TOKEN: &str = "BENCHER_API_TOKEN";
const BENCHER_PROJECT: &str = "BENCHER_PROJECT";
const BENCHER_BRANCH: &str = "BENCHER_BRANCH";
const BENCHER_TESTBED: &str = "BENCHER_TESTBED";

// Valid until 2027-09-05T19:03:59Z
const TEST_BENCHER_API_TOKEN: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjoxODIwMTcxMDM5LCJpYXQiOjE2NjIzODY0MDksImlzcyI6ImJlbmNoZXIuZGV2Iiwic3ViIjoibXVyaWVsLmJhZ2dlQG5vd2hlcmUuY29tIn0.sfAJmF9qIl_QRNnh8uLYuODHnxufXt_3m7skcNp1kMs";

// cargo test --features seed --test seed
#[test]
#[allow(clippy::too_many_lines, clippy::unwrap_used)]
fn test_cli_seed() -> Result<(), Box<dyn std::error::Error>> {
    // cargo run -- auth signup --host http://localhost:61016 --name "Eustace Bagge" eustace.bagge@nowhere.com
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "auth",
        "signup",
        HOST_ARG,
        LOCALHOST,
        "--name",
        "Eustace Bagge",
        "eustace.bagge@nowhere.com",
    ]);
    cmd.assert().success();

    // cargo run -- auth signup --host http://localhost:61016 --name "Muriel Bagge" muriel.bagge@nowhere.com
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "auth",
        "signup",
        HOST_ARG,
        LOCALHOST,
        "--name",
        "Muriel Bagge",
        "muriel.bagge@nowhere.com",
    ]);
    cmd.assert().success();

    // cargo run -- auth login --host http://localhost:61016 muriel.bagge@nowhere.com
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "auth",
        "login",
        HOST_ARG,
        LOCALHOST,
        "muriel.bagge@nowhere.com",
    ]);
    cmd.assert().success();

    // cargo run -- auth confirm --host http://localhost:61016 [AUTH_TOKEN]
    // cargo run -- token ls --host http://localhost:61016 --user muriel-bagge
    // cargo run -- token create --host http://localhost:61016 --user muriel-bagge --ttl 157784630 TEST_BENCHER_API_TOKEN

    // export BENCHER_API_TOKEN=[USER_TOKEN]
    env::set_var(BENCHER_API_TOKEN, TEST_BENCHER_API_TOKEN);

    // cargo run -- project ls --host http://localhost:61016 --org muriel-bagge
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "project",
        "ls",
        HOST_ARG,
        LOCALHOST,
        "--org",
        "muriel-bagge",
    ]);
    cmd.assert().success();

    // cargo run -- project create --host http://localhost:61016 --org muriel-bagge "The Computer"
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "project",
        "create",
        HOST_ARG,
        LOCALHOST,
        "--org",
        "muriel-bagge",
        "--slug",
        PROJECT_SLUG,
        "The Computer",
    ]);
    cmd.assert().success();

    // cargo run -- project view --host http://localhost:61016 --org muriel-bagge the-computer
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "project",
        "view",
        HOST_ARG,
        LOCALHOST,
        "--org",
        "muriel-bagge",
        PROJECT_SLUG,
    ]);
    cmd.assert().success();
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "project",
        "view",
        HOST_ARG,
        LOCALHOST,
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
        "--org",
        "muriel-bagge",
        PROJECT_SLUG,
    ]);
    cmd.assert().success();

    // export BENCHER_PROJECT=[PROJECT_UUID]
    let project = cmd.output().unwrap().stdout;
    let project: JsonProject = serde_json::from_slice(&project).unwrap();
    let project = project.uuid.to_string();
    env::set_var(BENCHER_PROJECT, project.clone());

    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "project",
        "ls",
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
        HOST_ARG,
        LOCALHOST,
        "--org",
        "muriel-bagge",
    ]);
    cmd.assert().success();

    // cargo run -- branch ls --host http://localhost:61016 --project the-computer
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "branch",
        "ls",
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    cmd.assert().success();

    // cargo run -- branch create --host http://localhost:61016 --project the-computer master
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "branch",
        "create",
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        PROJECT_SLUG,
        "--slug",
        BRANCH_SLUG,
        "master",
    ]);
    cmd.assert().success();

    // cargo run -- branch view --host http://localhost:61016 --project the-computer master
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "branch",
        "view",
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        PROJECT_SLUG,
        BRANCH_SLUG,
    ]);
    cmd.assert().success();

    // export BENCHER_BRANCH=[BRANCH_UUID]
    let branch = cmd.output().unwrap().stdout;
    let branch: JsonBranch = serde_json::from_slice(&branch).unwrap();
    let branch = branch.uuid.to_string();
    env::set_var(BENCHER_BRANCH, branch.clone());

    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "branch",
        "ls",
        HOST_ARG,
        LOCALHOST,
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    cmd.assert().success();

    // cargo run -- testbed ls --host http://localhost:61016 --project the-computer
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "testbed",
        "ls",
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    cmd.assert().success();

    // cargo run -- testbed create --host http://localhost:61016 --project the-computer base
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "testbed",
        "create",
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        PROJECT_SLUG,
        "--slug",
        TESTBED_SLUG,
        "base",
    ]);
    cmd.assert().success();

    // cargo run -- testbed view --host http://localhost:61016 --project the-computer base
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "testbed",
        "view",
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        PROJECT_SLUG,
        TESTBED_SLUG,
    ]);
    cmd.assert().success();

    // export BENCHER_TESTBED=[TESTBED_UUID]
    let testbed = cmd.output().unwrap().stdout;
    let testbed: JsonTestbed = serde_json::from_slice(&testbed).unwrap();
    let testbed = testbed.uuid.to_string();
    env::set_var(BENCHER_TESTBED, testbed.clone());

    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "testbed",
        "ls",
        HOST_ARG,
        LOCALHOST,
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    cmd.assert().success();

    // cargo run -- metric-kind ls --host http://localhost:61016 --project the-computer
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "metric-kind",
        "ls",
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    cmd.assert().success();

    // cargo run -- metric-kind create --host http://localhost:61016 --project the-computer --slug screams-888 --units "screams/minute" screams-888
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    let metric_kind = format!("screams-{}", rand::random::<u32>());
    cmd.args([
        "metric-kind",
        "create",
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        PROJECT_SLUG,
        "--slug",
        &metric_kind,
        "--units",
        "screams/minute",
        &metric_kind,
    ]);
    cmd.assert().success();

    // cargo run -- metric-kind view --host http://localhost:61016 --project the-computer screams-888
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "metric-kind",
        "view",
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        PROJECT_SLUG,
        &metric_kind,
    ]);
    cmd.assert().success();

    // // export BENCHER_TESTBED=[TESTBED_UUID]
    // let metric_kind = cmd.output().unwrap().stdout;
    // let metric_kind: JsonMetricKind = serde_json::from_slice(&metric_kind).unwrap();
    // let metric_kind = metric_kind.uuid.to_string();

    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "metric-kind",
        "ls",
        HOST_ARG,
        LOCALHOST,
        TOKEN_ARG,
        TEST_BENCHER_API_TOKEN,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    cmd.assert().success();

    // cargo run -- threshold ls --host http://localhost:61016 --project the-computer
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "threshold",
        "ls",
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    cmd.assert().success();

    // TODO: For some strange reason this always gets called twice...
    // cargo run -- threshold create --host http://localhost:61016 --metric-kind latency --branch $BENCHER_BRANCH --testbed $BENCHER_TESTBED --test z
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "threshold",
        "create",
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        &project,
        "--metric-kind",
        &metric_kind,
        BRANCH_ARG,
        &branch,
        TESTBED_ARG,
        &testbed,
        "--test",
        "z",
    ]);
    cmd.assert().success();

    // cargo run -- org ls --host http://localhost:61016
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args(["org", "ls", HOST_ARG, LOCALHOST]);
    cmd.assert().success();

    let org = cmd.output().unwrap().stdout;
    let mut org: Vec<JsonOrganization> = serde_json::from_slice(&org).unwrap();
    let org_uuid = org.pop().unwrap().uuid.to_string();

    // cargo run -- member invite --host http://localhost:61016 --email courage@nowhere.com --org <ORG_UUID>
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "member",
        "invite",
        HOST_ARG,
        LOCALHOST,
        "--email",
        "courage@nowhere.com",
        "--org",
        &org_uuid,
        "--role",
        "leader",
    ]);
    cmd.assert().success();

    Ok(())
}
