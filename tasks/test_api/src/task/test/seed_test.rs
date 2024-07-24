use std::process::Command;

use assert_cmd::{assert::OutputAssertExt, cargo::CommandCargoExt};
use bencher_json::{Jwt, Url, LOCALHOST_BENCHER_API_URL};
use pretty_assertions::assert_eq;

use crate::parser::TaskSeedTest;

const BENCHER_CMD: &str = "bencher";
const HOST_ARG: &str = "--host";
const TOKEN_ARG: &str = "--token";
const ORG_SLUG: &str = "muriel-bagge";
const PROJECT_ARG: &str = "--project";
const PROJECT_SLUG: &str = "the-computer";
const BRANCH_ARG: &str = "--branch";
const BRANCH_SLUG: &str = "master";
const TESTBED_ARG: &str = "--testbed";
const TESTBED_SLUG: &str = "base";
const MEASURE_ARG: &str = "--measure";
const MEASURE_SLUG: &str = "screams";

const CLI_DIR: &str = "./services/cli";

#[derive(Debug)]
pub struct SeedTest {
    pub url: Url,
    pub token: Jwt,
}

impl TryFrom<TaskSeedTest> for SeedTest {
    type Error = anyhow::Error;

    fn try_from(test: TaskSeedTest) -> Result<Self, Self::Error> {
        let TaskSeedTest { url, token } = test;
        Ok(Self {
            url: url.unwrap_or_else(|| LOCALHOST_BENCHER_API_URL.clone().into()),
            token: token.unwrap_or_else(Jwt::test_token),
        })
    }
}

impl SeedTest {
    #[allow(clippy::too_many_lines)]
    pub fn exec(&self) -> anyhow::Result<()> {
        let host = self.url.as_ref();
        let token = self.token.as_ref();

        // Signup Eustace Bagge first so he is admin
        // cargo run -- auth signup --host http://localhost:61016 --name "Eustace Bagge" eustace.bagge@nowhere.com
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "auth",
            "signup",
            HOST_ARG,
            host,
            "--name",
            "Eustace Bagge",
            "--i-agree",
            "eustace.bagge@nowhere.com",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonAuthAck =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- auth signup --host http://localhost:61016 --name "Muriel Bagge" muriel.bagge@nowhere.com
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "auth",
            "signup",
            HOST_ARG,
            host,
            "--name",
            "Muriel Bagge",
            "--i-agree",
            "muriel.bagge@nowhere.com",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonAuthAck =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- auth login --host http://localhost:61016 muriel.bagge@nowhere.com
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["auth", "login", HOST_ARG, host, "muriel.bagge@nowhere.com"])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonAuthAck =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- org ls --host http://localhost:61016 --token $BENCHER_API_TOKEN
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["org", "ls", HOST_ARG, host, TOKEN_ARG, token])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let organizations: bencher_json::JsonOrganizations =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(organizations.0.len(), 1);

        // cargo run -- org view --host http://localhost:61016 --token $BENCHER_API_TOKEN muriel-bagge
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["org", "view", HOST_ARG, host, TOKEN_ARG, token, ORG_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonOrganization =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- member invite --host http://localhost:61016 --token $BENCHER_API_TOKEN --name Courage --email courage@nowhere.com --role leader muriel-bagge
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "member",
            "invite",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            "--name",
            "Courage",
            "--email",
            "courage@nowhere.com",
            "--role",
            "leader",
            ORG_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonAuthAck =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- project ls --host http://localhost:61016
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["project", "ls", HOST_ARG, host])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let projects: bencher_json::JsonProjects =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(projects.0.len(), 0);

        // cargo run -- project ls --host http://localhost:61016 --token $BENCHER_API_TOKEN
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["project", "ls", HOST_ARG, host, TOKEN_ARG, token])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let projects: bencher_json::JsonProjects =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(projects.0.len(), 0);

        // cargo run -- project ls --host http://localhost:61016 --token $BENCHER_API_TOKEN muriel-bagge
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["project", "ls", HOST_ARG, host, TOKEN_ARG, token, ORG_SLUG])
            .current_dir(CLI_DIR);
        cmd.assert().success();
        let assert = cmd.assert().success();
        let projects: bencher_json::JsonProjects =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(projects.0.len(), 0);

        // cargo run -- project create --host http://localhost:61016 --token $BENCHER_API_TOKEN --name "The Computer" --slug the-computer muriel-bagge
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "project",
            "create",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            "--name",
            "The Computer",
            "--slug",
            PROJECT_SLUG,
            ORG_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonProject =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- project ls --host http://localhost:61016
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["project", "ls", HOST_ARG, host])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let projects: bencher_json::JsonProjects =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(projects.0.len(), 1);

        // cargo run -- project ls --host http://localhost:61016 --token $BENCHER_API_TOKEN
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["project", "ls", HOST_ARG, host, TOKEN_ARG, token])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let projects: bencher_json::JsonProjects =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(projects.0.len(), 1);

        // cargo run -- project ls --host http://localhost:61016 --token $BENCHER_API_TOKEN muriel-bagge
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["project", "ls", HOST_ARG, host, TOKEN_ARG, token, ORG_SLUG])
            .current_dir(CLI_DIR);
        cmd.assert().success();
        let assert = cmd.assert().success();
        let projects: bencher_json::JsonProjects =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(projects.0.len(), 1);

        // cargo run -- project view --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["project", "view", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonProject =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- project view --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "project",
            "view",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonProject =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- measure ls --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["measure", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let measures: bencher_json::JsonMeasures =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(measures.0.len(), 2);

        // cargo run -- measure ls --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "measure",
            "ls",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let measures: bencher_json::JsonMeasures =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(measures.0.len(), 2);

        // cargo run -- measure create --host http://localhost:61016 --token $BENCHER_API_TOKEN --name Screams --slug screams --units "Decibels (dB)" the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "measure",
            "create",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            "--name",
            "Screams",
            "--slug",
            MEASURE_SLUG,
            "--units",
            "Decibels (dB)",
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonMeasure =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- measure ls --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["measure", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let measures: bencher_json::JsonMeasures =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(measures.0.len(), 3);

        // cargo run -- measure ls --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "measure",
            "ls",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let measures: bencher_json::JsonMeasures =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(measures.0.len(), 3);

        // cargo run -- measure view --host http://localhost:61016 the-computer screams
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "measure",
            "view",
            HOST_ARG,
            host,
            PROJECT_SLUG,
            MEASURE_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonMeasure =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- measure view --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer screams
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "measure",
            "view",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
            MEASURE_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonMeasure =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- branch ls --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["branch", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let branches: bencher_json::JsonBranches =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(branches.0.len(), 1);

        // cargo run -- branch ls --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "branch",
            "ls",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let branches: bencher_json::JsonBranches =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(branches.0.len(), 1);

        // cargo run -- branch create --host http://localhost:61016 --token $BENCHER_API_TOKEN --name master --slug master the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "branch",
            "create",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            "--name",
            "master",
            "--slug",
            BRANCH_SLUG,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonBranch =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- branch ls --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["branch", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let branches: bencher_json::JsonBranches =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(branches.0.len(), 2);

        // cargo run -- branch ls --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "branch",
            "ls",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let branches: bencher_json::JsonBranches =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(branches.0.len(), 2);

        // cargo run -- branch view --host http://localhost:61016 the-computer master
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["branch", "view", HOST_ARG, host, PROJECT_SLUG, BRANCH_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonBranch =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- branch view --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer master
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "branch",
            "view",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
            BRANCH_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonBranch =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- testbed ls --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["testbed", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let testbeds: bencher_json::JsonTestbeds =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(testbeds.0.len(), 1);

        // cargo run -- testbed ls --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "testbed",
            "ls",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let testbeds: bencher_json::JsonTestbeds =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(testbeds.0.len(), 1);

        // cargo run -- testbed create --host http://localhost:61016  --name Base --slug base the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "testbed",
            "create",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            "--name",
            "Base",
            "--slug",
            TESTBED_SLUG,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonTestbed =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- testbed ls --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["testbed", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let testbeds: bencher_json::JsonTestbeds =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(testbeds.0.len(), 2);

        // cargo run -- testbed ls --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "testbed",
            "ls",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let testbeds: bencher_json::JsonTestbeds =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(testbeds.0.len(), 2);

        // cargo run -- testbed view --host http://localhost:61016 the-computer base
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "testbed",
            "view",
            HOST_ARG,
            host,
            PROJECT_SLUG,
            TESTBED_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonTestbed =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- testbed view --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer base
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "testbed",
            "view",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
            TESTBED_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonTestbed =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- threshold ls --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["threshold", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let thresholds: bencher_json::JsonThresholds =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(thresholds.0.len(), 2);

        // cargo run -- threshold ls --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "threshold",
            "ls",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let thresholds: bencher_json::JsonThresholds =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(thresholds.0.len(), 2);

        // cargo run -- threshold create --host http://localhost:61016 --token $BENCHER_API_TOKEN --branch master --testbed base --measure latency --test t --upper-boundary 0.995 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "threshold",
            "create",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            BRANCH_ARG,
            BRANCH_SLUG,
            TESTBED_ARG,
            TESTBED_SLUG,
            MEASURE_ARG,
            "latency",
            "--test",
            "t",
            "--upper-boundary",
            "0.995",
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let threshold: bencher_json::JsonThreshold =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- threshold ls --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["threshold", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let thresholds: bencher_json::JsonThresholds =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(thresholds.0.len(), 3);

        // cargo run -- threshold ls --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "threshold",
            "ls",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let thresholds: bencher_json::JsonThresholds =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(thresholds.0.len(), 3);

        // cargo run -- threshold view --host http://localhost:61016 the-computer [threshold.uuid]
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "threshold",
            "view",
            HOST_ARG,
            host,
            PROJECT_SLUG,
            &threshold.uuid.to_string(),
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonThreshold =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- threshold view --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer [threshold.uuid]
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "threshold",
            "view",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
            &threshold.uuid.to_string(),
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonThreshold =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- alert ls --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        // cargo run -- alert ls --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "ls",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        for i in 0..30i32 {
            let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
            let bencher_cmd = cmd.get_program().to_string_lossy().to_string();
            if i.rem_euclid(2) == 0 {
                // cargo run -- run --host http://localhost:61016 --token $BENCHER_API_TOKEN --project the-computer --branch master --testbed base --quiet "bencher mock"
                cmd.args([
                    "run",
                    HOST_ARG,
                    host,
                    TOKEN_ARG,
                    token,
                    PROJECT_ARG,
                    PROJECT_SLUG,
                    BRANCH_ARG,
                    BRANCH_SLUG,
                    TESTBED_ARG,
                    TESTBED_SLUG,
                    "--quiet",
                    &format!("{bencher_cmd} mock"),
                ])
            } else {
                // cargo run -- run --host http://localhost:61016 --token $BENCHER_API_TOKEN --project the-computer --branch master --testbed base --quiet bencher mock
                cmd.args([
                    "run",
                    HOST_ARG,
                    host,
                    TOKEN_ARG,
                    token,
                    PROJECT_ARG,
                    PROJECT_SLUG,
                    BRANCH_ARG,
                    BRANCH_SLUG,
                    TESTBED_ARG,
                    TESTBED_SLUG,
                    "--quiet",
                    &bencher_cmd,
                    "mock",
                ])
            }
            .current_dir(CLI_DIR);
            let assert = cmd.assert().success();
            let _json: bencher_json::JsonReport =
                serde_json::from_slice(&assert.get_output().stdout).unwrap();
        }

        // cargo run -- alert ls --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "ls",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        // cargo run -- run --host http://localhost:61016 --token $BENCHER_API_TOKEN --project the-computer --branch master --testbed base --quiet bencher mock --pow 10
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        let bencher_cmd = cmd.get_program().to_string_lossy().to_string();
        cmd.args([
            "run",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_ARG,
            PROJECT_SLUG,
            BRANCH_ARG,
            BRANCH_SLUG,
            TESTBED_ARG,
            TESTBED_SLUG,
            "--quiet",
            &bencher_cmd,
            "mock",
            "--pow",
            "10",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- alert ls --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 5);

        // cargo run -- alert ls --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "ls",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 5);

        // cargo run -- alert get --host http://localhost:61016 the-computer [alert.uuid]
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "view",
            HOST_ARG,
            host,
            PROJECT_SLUG,
            #[allow(clippy::indexing_slicing)]
            alerts.0[0].uuid.to_string().as_str(),
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _alert: bencher_json::JsonAlert =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- alert get --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer [alert.uuid]
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "view",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
            #[allow(clippy::indexing_slicing)]
            alerts.0[0].uuid.to_string().as_str(),
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _alert: bencher_json::JsonAlert =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- run --host http://localhost:61016 --token $BENCHER_API_TOKEN --project the-computer --branch feature-version --branch-start-point master --testbed base --quiet bencher mock
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        let bencher_cmd = cmd.get_program().to_string_lossy().to_string();
        cmd.args([
            "run",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_ARG,
            PROJECT_SLUG,
            BRANCH_ARG,
            "feature-version",
            "--branch-start-point",
            BRANCH_SLUG,
            TESTBED_ARG,
            TESTBED_SLUG,
            "--quiet",
            &bencher_cmd,
            "mock",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- alert ls --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 5);

        // cargo run -- alert ls --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "ls",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 5);

        // TODO reenable once semantics have been changed
        // https://github.com/bencherdev/bencher/issues/450
        // // cargo run -- run --host http://localhost:61016 --token $BENCHER_API_TOKEN --project the-computer --branch feature-hash --branch-start-point master --branch-start-point-hash df13bc928cc205cb8737e63b97712ba8d7d51b8b --testbed base --quiet bencher mock
        // let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        // let bencher_cmd = cmd.get_program().to_string_lossy().to_string();
        // cmd.args([
        //     "run",
        //     HOST_ARG,
        //     host,
        //     TOKEN_ARG,
        //     token,
        //     PROJECT_ARG,
        //     PROJECT_SLUG,
        //     BRANCH_ARG,
        //     "feature-hash",
        //     "--branch-start-point",
        //     BRANCH_SLUG,
        //     "--branch-start-point-hash",
        //     "df13bc928cc205cb8737e63b97712ba8d7d51b8b",
        //     TESTBED_ARG,
        //     TESTBED_SLUG,
        //     "--quiet",
        //     &bencher_cmd,
        //     "mock",
        // ])
        // .current_dir(CLI_DIR);
        // let assert = cmd.assert().success();
        // let _json: bencher_json::JsonReport =
        //     serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // // cargo run -- alert ls --host http://localhost:61016 the-computer
        // let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        // cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
        //     .current_dir(CLI_DIR);
        // let assert = cmd.assert().success();
        // let alerts: bencher_json::JsonAlerts =
        //     serde_json::from_slice(&assert.get_output().stdout).unwrap();
        // assert_eq!(alerts.0.len(), 5);

        // // cargo run -- alert ls --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer
        // let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        // cmd.args([
        //     "alert",
        //     "ls",
        //     HOST_ARG,
        //     host,
        //     TOKEN_ARG,
        //     token,
        //     PROJECT_SLUG,
        // ])
        // .current_dir(CLI_DIR);
        // let assert = cmd.assert().success();
        // let alerts: bencher_json::JsonAlerts =
        //     serde_json::from_slice(&assert.get_output().stdout).unwrap();
        // assert_eq!(alerts.0.len(), 5);

        // cargo run -- run --host http://localhost:61016 --token $BENCHER_API_TOKEN --project the-computer --branch master --branch-reset --testbed base --quiet bencher mock
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        let bencher_cmd = cmd.get_program().to_string_lossy().to_string();
        cmd.args([
            "run",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_ARG,
            PROJECT_SLUG,
            BRANCH_ARG,
            BRANCH_SLUG,
            "--branch-reset",
            TESTBED_ARG,
            TESTBED_SLUG,
            "--quiet",
            &bencher_cmd,
            "mock",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- alert ls --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        // cargo run -- alert ls --host http://localhost:61016 --token $BENCHER_API_TOKEN the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "ls",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        Ok(())
    }
}
