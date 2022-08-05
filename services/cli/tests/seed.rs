#![cfg(feature = "seed")]

use std::process::Command;

use assert_cmd::prelude::*;
use bencher_json::JsonUser;

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

    let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
    cmd.args(["project", "ls", TOKEN_ARG, &token, HOST_ARG, LOCALHOST]);
    cmd.assert().success();

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

    Ok(())
}
