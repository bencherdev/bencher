#![cfg(feature = "seed")]

use std::process::Command;

use assert_cmd::prelude::*; // Add methods on commands
use predicates::prelude::*; // Used for writing assertions
use pretty_assertions::assert_eq; // Run programs

#[test]
fn test_cli_seed() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("bencher")?;
    cmd.args([
        "auth",
        "signup",
        "--host",
        "http://localhost:8080",
        "--name",
        r#""Eustace Bagge""#,
        "eustace.bagge@nowhere.com",
    ]);
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("bencher")?;
    cmd.args([
        "auth",
        "signup",
        "--host",
        "http://localhost:8080",
        "--name",
        r#""Muriel Bagge""#,
        "muriel.bagge@nowhere.com",
    ]);
    cmd.assert().success();

    Ok(())
}
