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
const HASH_ARG: &str = "--hash";
const TESTBED_ARG: &str = "--testbed";
const TESTBED_SLUG: &str = "base";
const MEASURE_ARG: &str = "--measure";
const MEASURE_SLUG: &str = "screams";

const REPO_NAME: &str = "bencher";
const UNCLAIMED_SLUG: &str = "unclaimed";
const CLAIMED_SLUG: &str = "claimed";

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
        let json: bencher_json::JsonOrganization =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        let muriel_bagge_org_uuid = json.uuid;

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

        // cargo run -- threshold create --host http://localhost:61016 --token $BENCHER_API_TOKEN --branch master --testbed base --measure latency --test t --upper-boundary 0.99 the-computer
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
            "t_test",
            "--upper-boundary",
            "0.99",
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

        let mut hash = Hash::new();
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
                    HASH_ARG,
                    &hash.next(),
                    TESTBED_ARG,
                    TESTBED_SLUG,
                    "--format",
                    "json",
                    "--quiet",
                    &format!("{bencher_cmd} mock --measure latency --measure {MEASURE_SLUG}"),
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
                    "--no-hash",
                    TESTBED_ARG,
                    TESTBED_SLUG,
                    "--format",
                    "json",
                    "--quiet",
                    &bencher_cmd,
                    "mock",
                    "--measure",
                    "latency",
                    "--measure",
                    MEASURE_SLUG,
                ])
            }
            .current_dir(CLI_DIR);
            let assert = cmd.assert().success();
            let _json: bencher_json::JsonReport =
                serde_json::from_slice(&assert.get_output().stdout).unwrap();
        }

        // cargo run -- alert ls --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        std::thread::sleep(std::time::Duration::from_secs(1));

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
            HASH_ARG,
            &hash.next(),
            TESTBED_ARG,
            TESTBED_SLUG,
            "--threshold-measure",
            MEASURE_SLUG,
            "--threshold-test",
            "t_test",
            "--threshold-upper-boundary",
            "0.98",
            "--thresholds-reset",
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--measure",
            MEASURE_SLUG,
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

        std::thread::sleep(std::time::Duration::from_secs(1));

        // cargo run -- run --host http://localhost:61016 --token $BENCHER_API_TOKEN --project the-computer --branch feature-branch --branch-start-point master --testbed base --quiet bencher mock
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
            "feature-branch",
            HASH_ARG,
            &hash.next(),
            "--start-point",
            BRANCH_SLUG,
            "--start-point-clone-thresholds",
            "--start-point-reset",
            TESTBED_ARG,
            TESTBED_SLUG,
            "--format",
            "json",
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

        std::thread::sleep(std::time::Duration::from_secs(1));

        // This shouldn't generate any alerts because we reset the thresholds on the start point branch
        // cargo run -- run --host http://localhost:61016 --token $BENCHER_API_TOKEN --project the-computer --branch feature-branch --branch-start-point master --testbed base --quiet bencher mock
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
            "feature-branch",
            HASH_ARG,
            &hash.next(),
            "--start-point",
            BRANCH_SLUG,
            "--start-point-clone-thresholds",
            "--start-point-reset",
            TESTBED_ARG,
            TESTBED_SLUG,
            "--format",
            "json",
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
        assert_eq!(alerts.0.len(), 0);

        std::thread::sleep(std::time::Duration::from_secs(1));

        // This should generate alerts because we are using the measure that has a thresholds set for it on the start point branch
        // cargo run -- run --host http://localhost:61016 --token $BENCHER_API_TOKEN --project the-computer --branch feature-branch --branch-start-point master --testbed base --quiet bencher mock
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
            "feature-branch",
            HASH_ARG,
            &hash.next(),
            "--start-point",
            BRANCH_SLUG,
            "--start-point-clone-thresholds",
            "--start-point-reset",
            TESTBED_ARG,
            TESTBED_SLUG,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--measure",
            MEASURE_SLUG,
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

        std::thread::sleep(std::time::Duration::from_secs(1));

        // Reset the feature branch
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
            "feature-branch",
            HASH_ARG,
            &hash.next(),
            "--start-point-reset",
            TESTBED_ARG,
            TESTBED_SLUG,
            "--format",
            "json",
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

        // These alerts should be silenced
        // cargo run -- alert ls --host http://localhost:61016 the-computer --status active
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "ls",
            HOST_ARG,
            host,
            PROJECT_SLUG,
            "--status",
            "active",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        // cargo run -- alert ls --host http://localhost:61016 the-computer --status silenced
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "ls",
            HOST_ARG,
            host,
            PROJECT_SLUG,
            "--status",
            "silenced",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 5);

        // Archive the `feature-branch` branch
        // cargo run -- archive --host http://localhost:61016 --token $BENCHER_API_TOKEN --project the-computer --branch feature-branch
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "archive",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_ARG,
            PROJECT_SLUG,
            BRANCH_ARG,
            "feature-branch",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        assert_eq!(
            String::from_utf8_lossy(&assert.get_output().stdout),
            "Successfully archived the branch (feature-branch).\n"
        );

        // cargo run -- alert ls --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        // These alerts should be silenced
        // cargo run -- alert ls --host http://localhost:61016 the-computer --status active
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "ls",
            HOST_ARG,
            host,
            PROJECT_SLUG,
            "--status",
            "active",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        // cargo run -- alert ls --host http://localhost:61016 the-computer --status silenced
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "ls",
            HOST_ARG,
            host,
            PROJECT_SLUG,
            "--status",
            "silenced",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        // cargo run -- alert ls --host http://localhost:61016 the-computer --status silenced
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "ls",
            HOST_ARG,
            host,
            PROJECT_SLUG,
            "--status",
            "silenced",
            "--archived",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 5);

        // Unarchive the `feature-branch` branch
        // cargo run -- archive --host http://localhost:61016 --token $BENCHER_API_TOKEN --project the-computer --branch feature-branch
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "unarchive",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_ARG,
            PROJECT_SLUG,
            BRANCH_ARG,
            "feature-branch",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        assert_eq!(
            String::from_utf8_lossy(&assert.get_output().stdout),
            "Successfully unarchived the branch (feature-branch).\n"
        );

        // cargo run -- alert ls --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 5);

        // These alerts should be silenced
        // cargo run -- alert ls --host http://localhost:61016 the-computer --status active
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "ls",
            HOST_ARG,
            host,
            PROJECT_SLUG,
            "--status",
            "active",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        // cargo run -- alert ls --host http://localhost:61016 the-computer --status silenced
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "ls",
            HOST_ARG,
            host,
            PROJECT_SLUG,
            "--status",
            "silenced",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 5);

        // cargo run -- alert ls --host http://localhost:61016 the-computer --status silenced
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "ls",
            HOST_ARG,
            host,
            PROJECT_SLUG,
            "--status",
            "silenced",
            "--archived",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        std::thread::sleep(std::time::Duration::from_secs(1));

        // cargo run -- alert ls --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 5);

        std::thread::sleep(std::time::Duration::from_secs(1));

        // Generate alerts on the master branch using already defined Thresholds
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
            HASH_ARG,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            TESTBED_ARG,
            TESTBED_SLUG,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--measure",
            MEASURE_SLUG,
            "--pow",
            "10",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- alert ls --host http://localhost:61016 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "ls",
            HOST_ARG,
            host,
            "--per-page",
            "255",
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 10);

        // cargo run -- alert ls --host http://localhost:61016 the-computer --status active
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "ls",
            HOST_ARG,
            host,
            PROJECT_SLUG,
            "--status",
            "active",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 5);

        // cargo run -- alert ls --host http://localhost:61016 the-computer --status silenced
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "ls",
            HOST_ARG,
            host,
            PROJECT_SLUG,
            "--status",
            "silenced",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 5);

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

        std::thread::sleep(std::time::Duration::from_secs(1));

        // If the start point is missing, then the branch should just be reset
        // https://github.com/bencherdev/bencher/issues/450
        // cargo run -- run --host http://localhost:61016 --token $BENCHER_API_TOKEN --project the-computer --branch feature-hash --branch-start-point master --branch-start-point-hash badbadbadbadbadbadbadbadbadbadbadbadbad1 --testbed base --quiet bencher mock
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
            "feature-hash",
            HASH_ARG,
            &hash.next(),
            "--start-point",
            BRANCH_SLUG,
            "--start-point-hash",
            "badbadbadbadbadbadbadbadbadbadbadbadbad1",
            TESTBED_ARG,
            TESTBED_SLUG,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert!(json.branch.head.start_point.is_none(), "{json:?}");

        std::thread::sleep(std::time::Duration::from_secs(1));

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
            "feature-hash",
            HASH_ARG,
            &hash.next(),
            "--start-point",
            BRANCH_SLUG,
            "--start-point-hash",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            TESTBED_ARG,
            TESTBED_SLUG,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(
            json.branch
                .head
                .start_point
                .unwrap()
                .version
                .hash
                .unwrap()
                .as_ref(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );

        std::thread::sleep(std::time::Duration::from_secs(1));

        // Anonymous report
        // It should use the same on-the-fly project across multiple runs
        let mut anonymous_project: Option<bencher_json::JsonProject> = None;
        for _ in 0..3 {
            let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
            let bencher_cmd = cmd.get_program().to_string_lossy().to_string();
            cmd.args([
                "run",
                HOST_ARG,
                host,
                "--format",
                "json",
                "--quiet",
                &bencher_cmd,
                "mock",
            ])
            .current_dir(CLI_DIR);
            let assert = cmd.assert().success();
            let json: bencher_json::JsonReport =
                serde_json::from_slice(&assert.get_output().stdout).unwrap();

            if let Some(project) = &anonymous_project {
                assert_eq!(json.project.uuid, project.uuid);
                assert_eq!(json.project.organization, project.organization);
                assert_eq!(json.project.name, project.name);
                assert_eq!(json.project.slug, project.slug);
                assert_eq!(json.project.claimed, project.claimed);
            } else {
                assert_eq!(json.project.name.as_ref(), REPO_NAME);
                assert!(
                    json.project.slug.as_ref().starts_with(REPO_NAME),
                    "{json:?}"
                );
                assert_eq!(
                    json.project.slug.as_ref().len(),
                    REPO_NAME.len() + 1 + 7 + 1 + 13
                );
                assert_eq!(json.project.claimed, None);
                anonymous_project.replace(json.project);
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(1));

        // Anonymous report with project slug
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        let bencher_cmd = cmd.get_program().to_string_lossy().to_string();
        cmd.args([
            "run",
            HOST_ARG,
            host,
            PROJECT_ARG,
            UNCLAIMED_SLUG,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(json.project.name.as_ref(), REPO_NAME);
        assert_eq!(json.project.slug.as_ref(), UNCLAIMED_SLUG);
        assert_eq!(json.project.claimed, None);
        let organization_uuid = json.project.organization;
        let organization_uuid_str = organization_uuid.to_string();

        std::thread::sleep(std::time::Duration::from_secs(1));

        // Claim the organization
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "organization",
            "claim",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            &organization_uuid_str,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonOrganization =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(json.uuid, organization_uuid);
        assert_eq!(json.name.as_ref(), REPO_NAME);
        assert_eq!(json.slug.as_ref(), UNCLAIMED_SLUG);
        assert!(json.claimed.is_some(), "{json:?}");

        std::thread::sleep(std::time::Duration::from_secs(1));

        // Claimed report with project slug
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        let bencher_cmd = cmd.get_program().to_string_lossy().to_string();
        cmd.args([
            "run",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_ARG,
            UNCLAIMED_SLUG,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(json.project.organization, organization_uuid);
        assert_eq!(json.project.name.as_ref(), REPO_NAME);
        assert_eq!(json.project.slug.as_ref(), UNCLAIMED_SLUG);
        assert!(json.project.claimed.is_some(), "{json:?}");

        std::thread::sleep(std::time::Duration::from_secs(1));

        // On-the-fly project for user
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        let bencher_cmd = cmd.get_program().to_string_lossy().to_string();
        cmd.args([
            "run",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        let anonymous_project = anonymous_project.unwrap();
        assert_eq!(json.project.organization, anonymous_project.organization);
        assert_eq!(json.project.name, anonymous_project.name);
        assert_eq!(json.project.slug, anonymous_project.slug);
        assert!(json.project.claimed.is_some(), "{json:?}");

        std::thread::sleep(std::time::Duration::from_secs(1));

        // On-the-fly project for user with project slug
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        let bencher_cmd = cmd.get_program().to_string_lossy().to_string();
        cmd.args([
            "run",
            HOST_ARG,
            host,
            TOKEN_ARG,
            token,
            PROJECT_ARG,
            CLAIMED_SLUG,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(json.project.organization, muriel_bagge_org_uuid);
        assert_eq!(json.project.name.as_ref(), REPO_NAME);
        assert_eq!(json.project.slug.as_ref(), CLAIMED_SLUG);
        assert!(json.project.claimed.is_some(), "{json:?}");

        Ok(())
    }
}

struct Hash {
    count: u8,
}

impl Hash {
    fn new() -> Self {
        Self { count: 0 }
    }

    fn next(&mut self) -> String {
        let mock_hash = self
            .count
            .to_string()
            .repeat(if self.count > 9 { 20 } else { 40 });
        self.count += 1;
        mock_hash
    }
}
