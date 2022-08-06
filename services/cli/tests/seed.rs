#![cfg(feature = "seed")]

use std::process::Command;

use assert_cmd::prelude::*;
use bencher_json::{
    JsonBranch,
    JsonTestbed,
    JsonUser,
};

const BENCHER_CMD: &str = "bencher";
const HOST_ARG: &str = "--host";
const LOCALHOST: &str = "http://localhost:8080";
const TOKEN_ARG: &str = "--token";
const PROJECT_ARG: &str = "--project";
const PROJECT_SLUG: &str = "the-computer";
const BRANCH_SLUG: &str = "master";
const TESTBED_SLUG: &str = "base";

// cargo test --features seed --test seed
#[test]
fn test_cli_seed() -> Result<(), Box<dyn std::error::Error>> {
    // cargo run -- auth signup --host http://localhost:8080 --name "Eustace Bagge" eustace.bagge@nowhere.com
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "auth",
        "signup",
        HOST_ARG,
        LOCALHOST,
        "--name",
        r#""Eustace Bagge""#,
        "eustace.bagge@nowhere.com",
    ]);
    cmd.assert().success();

    // cargo run -- auth signup --host http://localhost:8080 --name "Muriel Bagge" muriel.bagge@nowhere.com
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "auth",
        "signup",
        HOST_ARG,
        LOCALHOST,
        "--name",
        r#""Muriel Bagge""#,
        "muriel.bagge@nowhere.com",
    ]);
    cmd.assert().success();

    // cargo run -- auth login --host http://localhost:8080 muriel.bagge@nowhere.com
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "auth",
        "login",
        HOST_ARG,
        LOCALHOST,
        "muriel.bagge@nowhere.com",
    ]);
    cmd.assert().success();

    let login = cmd.output().unwrap().stdout;
    let login: JsonUser = serde_json::from_slice(&login).unwrap();
    let token = login.uuid.to_string();

    // cargo run -- project ls --host http://localhost:8080
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args(["project", "ls", TOKEN_ARG, &token, HOST_ARG, LOCALHOST]);
    cmd.assert().success();

    // cargo run -- project create --host http://localhost:8080 "The Computer"
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "project",
        "create",
        TOKEN_ARG,
        &token,
        HOST_ARG,
        LOCALHOST,
        "--slug",
        PROJECT_SLUG,
        "The Computer",
    ]);
    cmd.assert().success();

    // cargo run -- project view --host http://localhost:8080 the-computer
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "project",
        "view",
        TOKEN_ARG,
        &token,
        HOST_ARG,
        LOCALHOST,
        PROJECT_SLUG,
    ]);
    cmd.assert().success();

    // cargo run -- branch ls --host http://localhost:8080 --project the-computer
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "branch",
        "ls",
        TOKEN_ARG,
        &token,
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    cmd.assert().success();

    // cargo run -- branch create --host http://localhost:8080 --project the-computer master
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "branch",
        "create",
        TOKEN_ARG,
        &token,
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        PROJECT_SLUG,
        "--slug",
        BRANCH_SLUG,
        "master",
    ]);
    cmd.assert().success();

    // cargo run -- branch view --host http://localhost:8080 --project the-computer master
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "branch",
        "view",
        TOKEN_ARG,
        &token,
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        PROJECT_SLUG,
        BRANCH_SLUG,
    ]);
    cmd.assert().success();

    let branch = cmd.output().unwrap().stdout;
    let branch: JsonBranch = serde_json::from_slice(&branch).unwrap();
    let branch = branch.uuid.to_string();

    // cargo run -- testbed ls --host http://localhost:8080 --project the-computer
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "testbed",
        "ls",
        TOKEN_ARG,
        &token,
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    cmd.assert().success();

    // cargo run -- testbed create --host http://localhost:8080 --project the-computer base
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "testbed",
        "create",
        TOKEN_ARG,
        &token,
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        PROJECT_SLUG,
        "--slug",
        TESTBED_SLUG,
        "base",
    ]);
    cmd.assert().success();

    // cargo run -- testbed view --host http://localhost:8080 --project the-computer base
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "testbed",
        "view",
        TOKEN_ARG,
        &token,
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        PROJECT_SLUG,
        TESTBED_SLUG,
    ]);
    cmd.assert().success();

    let testbed = cmd.output().unwrap().stdout;
    let testbed: JsonTestbed = serde_json::from_slice(&testbed).unwrap();
    let testbed = testbed.uuid.to_string();

    // cargo run -- report ls --host http://localhost:8080 --project the-computer
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args([
        "report",
        "ls",
        TOKEN_ARG,
        &token,
        HOST_ARG,
        LOCALHOST,
        PROJECT_ARG,
        PROJECT_SLUG,
    ]);
    cmd.assert().success();

    // cargo run -- run --host http://localhost:8080 --adapter rust "cargo bench"
    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args(["run", TOKEN_ARG, &token, HOST_ARG, LOCALHOST]);
    cmd.assert().success();

    Ok(())
}
