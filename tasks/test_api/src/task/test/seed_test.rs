use std::process::Command;

use assert_cmd::{assert::OutputAssertExt as _, cargo::CommandCargoExt as _};
use bencher_json::Url;
use pretty_assertions::assert_eq;

use crate::{
    parser::TaskSeedTest,
    task::{
        session::{ApiLog, BrowserSession, Readers},
        unwrap_url,
    },
};

pub(crate) const BENCHER_CMD: &str = "bencher";
pub(crate) const HOST_ARG: &str = "--host";
pub(crate) const KEY_ARG: &str = "--key";
const ORG_SLUG: &str = "muriel-bagge";
/// Muriel's personal org slug doubles as her user slug.
pub(crate) const USER_SLUG: &str = ORG_SLUG;
pub(crate) const ADMIN_SLUG: &str = "eustace-bagge";
pub(crate) const PROJECT_ARG: &str = "--project";
pub(crate) const PROJECT_SLUG: &str = "the-computer";
pub(crate) const BRANCH_ARG: &str = "--branch";
pub(crate) const BRANCH_SLUG: &str = "master";
const HASH_ARG: &str = "--hash";
pub(crate) const TESTBED_ARG: &str = "--testbed";
pub(crate) const TESTBED_SLUG: &str = "base";
const MEASURE_ARG: &str = "--measure";
const MEASURE_SLUG: &str = "screams";

pub(crate) const ADMIN_EMAIL: &str = "eustace.bagge@nowhere.com";
pub(crate) const ADMIN_NAME: &str = "Eustace Bagge";
pub(crate) const USER_EMAIL: &str = "muriel.bagge@nowhere.com";
pub(crate) const USER_NAME: &str = "Muriel Bagge";
const INVITEE_EMAIL: &str = "katz@nowhere.com";

const REPO_NAME: &str = "bencher";
const NO_GIT_NAME: &str = "Project";
const UNCLAIMED_SLUG: &str = "unclaimed";
const CLAIMED_SLUG: &str = "claimed";
const PROJECT_KEY_NAME: &str = "seed-test-key";
const PROJECT_KEY_UPDATED_NAME: &str = "seed-test-key-updated";
const USER_KEY_NAME: &str = "seed-test-user-key";
const USER_KEY_UPDATED_NAME: &str = "seed-test-user-key-updated";

pub(crate) const CLI_DIR: &str = "./services/cli";

// https://courage.fandom.com/wiki/Perfect#Plot
const PERFECT_SEED: &str = "6";

// Each reset stands for its own CI run, so none takes the start point idempotency key
// of the GitHub Actions run that the test may run in.
const GITHUB_ACTIONS: &str = "GITHUB_ACTIONS";

#[derive(Debug)]
pub struct SeedTest {
    pub url: Url,
    pub is_bencher_cloud: bool,
    pub no_git: bool,
    pub api_log: ApiLog,
}

impl TryFrom<TaskSeedTest> for SeedTest {
    type Error = anyhow::Error;

    fn try_from(test: TaskSeedTest) -> Result<Self, Self::Error> {
        let TaskSeedTest {
            url,
            is_bencher_cloud,
            no_git,
            api_log,
        } = test;

        let url = unwrap_url(url);
        let api_log = ApiLog::new(&url, api_log)?;

        Ok(Self {
            url,
            is_bencher_cloud,
            no_git,
            api_log,
        })
    }
}

impl SeedTest {
    fn project_name(&self) -> &str {
        if self.no_git { NO_GIT_NAME } else { REPO_NAME }
    }

    /// Seed the server, and return its readers, each signed in with a key of their own.
    pub fn exec(&self) -> anyhow::Result<Readers> {
        if self.is_bencher_cloud {
            println!("Running seed test as Bencher Cloud: {}", self.url);
        } else {
            println!("Running seed test as Bencher Self-Hosted: {}", self.url);
        }

        // Eustace Bagge signs in first, so he is the server admin.
        let readers = Readers::sign_in(&self.url, &self.api_log)?;
        self.exec_with(&readers)?;
        Ok(readers)
    }

    #[expect(
        clippy::cognitive_complexity,
        clippy::panic_in_result_fn,
        clippy::too_many_lines,
        reason = "seed test exercises full API workflow"
    )]
    fn exec_with(&self, readers: &Readers) -> anyhow::Result<()> {
        let host = self.url.as_ref();
        let admin_key = readers.admin.key.as_ref();
        let key = readers.user.key.as_ref();
        let bencher_cmd = Command::cargo_bin(BENCHER_CMD)?
            .get_program()
            .to_string_lossy()
            .to_string();

        // cargo run -- org ls --host http://localhost:6610 --key $BENCHER_API_KEY
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["org", "ls", HOST_ARG, host, KEY_ARG, key])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let organizations: bencher_json::JsonOrganizations =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(organizations.0.len(), 1);

        // cargo run -- org view --host http://localhost:6610 --key $BENCHER_API_KEY muriel-bagge
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["org", "view", HOST_ARG, host, KEY_ARG, admin_key, ORG_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonOrganization =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        let admin_muriel_bagge_org_uuid = json.uuid;

        // cargo run -- org view --host http://localhost:6610 --key $BENCHER_API_KEY muriel-bagge
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["org", "view", HOST_ARG, host, KEY_ARG, key, ORG_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonOrganization =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        let muriel_bagge_org_uuid = json.uuid;
        assert_eq!(admin_muriel_bagge_org_uuid, muriel_bagge_org_uuid);

        // An invitation needs a browser session signed in within the last 10 minutes, and the
        // invitee accepts it with a session of their own, signed in by the emailed code.
        let secret = readers
            .user
            .session
            .invite(ORG_SLUG, INVITEE_EMAIL, "Katz")?;
        let organization = BrowserSession::sign_in(&self.url, INVITEE_EMAIL, &self.api_log)?
            .accept_invite(&secret)?;
        assert_eq!(organization.slug.to_string(), ORG_SLUG);

        // cargo run -- project ls --host http://localhost:6610 --key $BENCHER_API_KEY muriel-bagge
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["project", "ls", HOST_ARG, host, KEY_ARG, key, ORG_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let projects: bencher_json::JsonProjects =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(projects.0.len(), 0);

        // cargo run -- project create --host http://localhost:6610 --key $BENCHER_API_KEY --name "The Computer" --slug the-computer muriel-bagge
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "project",
            "create",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
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

        // cargo run -- project ls --host http://localhost:6610 --key $BENCHER_API_KEY muriel-bagge
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["project", "ls", HOST_ARG, host, KEY_ARG, key, ORG_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let projects: bencher_json::JsonProjects =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(projects.0.len(), 1);

        // cargo run -- project view --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["project", "view", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonProject =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- project view --host http://localhost:6610 --key $BENCHER_API_KEY the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "project",
            "view",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonProject =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- branch ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["branch", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let branches: bencher_json::JsonBranches =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(branches.0.len(), 0);

        // cargo run -- branch ls --host http://localhost:6610 --key $BENCHER_API_KEY the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["branch", "ls", HOST_ARG, host, KEY_ARG, key, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let branches: bencher_json::JsonBranches =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(branches.0.len(), 0);

        // cargo run -- branch create --host http://localhost:6610 --key $BENCHER_API_KEY --name master --slug master the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "branch",
            "create",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
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

        // cargo run -- branch ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["branch", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let branches: bencher_json::JsonBranches =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(branches.0.len(), 1);

        // cargo run -- branch ls --host http://localhost:6610 --key $BENCHER_API_KEY the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["branch", "ls", HOST_ARG, host, KEY_ARG, key, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let branches: bencher_json::JsonBranches =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(branches.0.len(), 1);

        // cargo run -- branch view --host http://localhost:6610 the-computer master
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["branch", "view", HOST_ARG, host, PROJECT_SLUG, BRANCH_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonBranch =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- branch view --host http://localhost:6610 --key $BENCHER_API_KEY the-computer master
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "branch",
            "view",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            PROJECT_SLUG,
            BRANCH_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonBranch =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- testbed ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["testbed", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let testbeds: bencher_json::JsonTestbeds =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(testbeds.0.len(), 0);

        // cargo run -- testbed ls --host http://localhost:6610 --key $BENCHER_API_KEY the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["testbed", "ls", HOST_ARG, host, KEY_ARG, key, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let testbeds: bencher_json::JsonTestbeds =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(testbeds.0.len(), 0);

        // cargo run -- testbed create --host http://localhost:6610  --name Base --slug base the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "testbed",
            "create",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
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

        // cargo run -- testbed ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["testbed", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let testbeds: bencher_json::JsonTestbeds =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(testbeds.0.len(), 1);

        // cargo run -- testbed ls --host http://localhost:6610 --key $BENCHER_API_KEY the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["testbed", "ls", HOST_ARG, host, KEY_ARG, key, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let testbeds: bencher_json::JsonTestbeds =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(testbeds.0.len(), 1);

        // cargo run -- testbed view --host http://localhost:6610 the-computer base
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

        // cargo run -- testbed view --host http://localhost:6610 --key $BENCHER_API_KEY the-computer base
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "testbed",
            "view",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            PROJECT_SLUG,
            TESTBED_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonTestbed =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- measure ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["measure", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let measures: bencher_json::JsonMeasures =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(measures.0.len(), 0);

        // cargo run -- measure ls --host http://localhost:6610 --key $BENCHER_API_KEY the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["measure", "ls", HOST_ARG, host, KEY_ARG, key, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let measures: bencher_json::JsonMeasures =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(measures.0.len(), 0);

        // cargo run -- measure create --host http://localhost:6610 --key $BENCHER_API_KEY --name Screams --slug screams --units "Decibels (dB)" the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "measure",
            "create",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
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

        // cargo run -- measure ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["measure", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let measures: bencher_json::JsonMeasures =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(measures.0.len(), 1);

        // cargo run -- measure ls --host http://localhost:6610 --key $BENCHER_API_KEY the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["measure", "ls", HOST_ARG, host, KEY_ARG, key, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let measures: bencher_json::JsonMeasures =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(measures.0.len(), 1);

        // cargo run -- measure view --host http://localhost:6610 the-computer screams
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

        // cargo run -- measure view --host http://localhost:6610 --key $BENCHER_API_KEY the-computer screams
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "measure",
            "view",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            PROJECT_SLUG,
            MEASURE_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonMeasure =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // A new key needs a browser session signed in within the last 10 minutes.
        let project_key_created = readers
            .user
            .session
            .create_project_key(PROJECT_SLUG, PROJECT_KEY_NAME)?;
        let project_key = project_key_created.key.as_ref().to_owned();
        let project_key_uuid_str = project_key_created.uuid.to_string();

        // cargo run -- run --host http://localhost:6610 --key $PROJECT_KEY --project the-computer --quiet bencher mock --seed 6
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            project_key.as_str(),
            PROJECT_ARG,
            PROJECT_SLUG,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--seed",
            PERFECT_SEED,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // bencher mock --seed 6 | bencher run --host http://localhost:6610 --key $PROJECT_KEY --project the-computer --format json --quiet
        let mock_output = Command::cargo_bin(BENCHER_CMD)?
            .args(["mock", "--seed", PERFECT_SEED])
            .current_dir(CLI_DIR)
            .output()?;
        assert!(mock_output.status.success(), "{mock_output:?}");
        let output = Command::cargo_bin(BENCHER_CMD)?
            .args([
                "run",
                HOST_ARG,
                host,
                KEY_ARG,
                project_key.as_str(),
                PROJECT_ARG,
                PROJECT_SLUG,
                "--format",
                "json",
                "--quiet",
            ])
            .current_dir(CLI_DIR)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write as _;
                if let Some(mut stdin) = child.stdin.take() {
                    stdin.write_all(&mock_output.stdout)?;
                }
                child.wait_with_output()
            })?;
        assert!(
            output.status.success(),
            "piped bencher run failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let _json: bencher_json::JsonReport = serde_json::from_slice(&output.stdout).unwrap();

        // cargo run -- threshold ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["threshold", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let thresholds: bencher_json::JsonThresholds =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(thresholds.0.len(), 0);

        // cargo run -- threshold ls --host http://localhost:6610 --key $BENCHER_API_KEY the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "threshold",
            "ls",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let thresholds: bencher_json::JsonThresholds =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(thresholds.0.len(), 0);

        // cargo run -- threshold create --host http://localhost:6610 --key $BENCHER_API_KEY --branch master --testbed base --measure latency --test t --upper-boundary 0.99 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "threshold",
            "create",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
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

        // cargo run -- threshold ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["threshold", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let thresholds: bencher_json::JsonThresholds =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(thresholds.0.len(), 1);

        // cargo run -- threshold ls --host http://localhost:6610 --key $BENCHER_API_KEY the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "threshold",
            "ls",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            PROJECT_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let thresholds: bencher_json::JsonThresholds =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(thresholds.0.len(), 1);

        // cargo run -- threshold view --host http://localhost:6610 the-computer [threshold.uuid]
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

        // cargo run -- threshold view --host http://localhost:6610 --key $BENCHER_API_KEY the-computer [threshold.uuid]
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "threshold",
            "view",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            PROJECT_SLUG,
            &threshold.uuid.to_string(),
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonThreshold =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- alert ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        // cargo run -- alert ls --host http://localhost:6610 --key $BENCHER_API_KEY the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, KEY_ARG, key, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        let mut hash = Hash::new();
        for i in 0..30i32 {
            let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
            if i.rem_euclid(4) == 0 {
                // Project key + hash + single-string command
                cmd.args([
                    "run",
                    HOST_ARG,
                    host,
                    KEY_ARG,
                    project_key.as_str(),
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
                    &format!("{bencher_cmd} mock --seed {PERFECT_SEED} --measure latency --measure {MEASURE_SLUG}"),
                ])
            } else if i.rem_euclid(4) == 1 {
                // Token + no hash + separate args
                cmd.args([
                    "run",
                    HOST_ARG,
                    host,
                    KEY_ARG,
                    key,
                    PROJECT_ARG,
                    PROJECT_SLUG,
                    BRANCH_ARG,
                    BRANCH_SLUG,
                    TESTBED_ARG,
                    TESTBED_SLUG,
                    "--format",
                    "json",
                    "--quiet",
                    &bencher_cmd,
                    "mock",
                    "--seed",
                    PERFECT_SEED,
                    "--measure",
                    "latency",
                    "--measure",
                    MEASURE_SLUG,
                ])
            } else if i.rem_euclid(4) == 2 {
                // Token + hash + single-string command
                cmd.args([
                    "run",
                    HOST_ARG,
                    host,
                    KEY_ARG,
                    key,
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
                    &format!("{bencher_cmd} mock --seed {PERFECT_SEED} --measure latency --measure {MEASURE_SLUG}"),
                ])
            } else {
                // Project key + no hash + separate args
                cmd.args([
                    "run",
                    HOST_ARG,
                    host,
                    KEY_ARG,
                    project_key.as_str(),
                    PROJECT_ARG,
                    PROJECT_SLUG,
                    BRANCH_ARG,
                    BRANCH_SLUG,
                    TESTBED_ARG,
                    TESTBED_SLUG,
                    "--format",
                    "json",
                    "--quiet",
                    &bencher_cmd,
                    "mock",
                    "--seed",
                    PERFECT_SEED,
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

        // cargo run -- alert ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        std::thread::sleep(std::time::Duration::from_secs(1));

        // cargo run -- run --host http://localhost:6610 --key $BENCHER_API_KEY --project the-computer --branch master --branch-reset --testbed base --quiet bencher mock
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
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
            "--seed",
            PERFECT_SEED,
            "--measure",
            "latency",
            "--measure",
            MEASURE_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- alert ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        std::thread::sleep(std::time::Duration::from_secs(1));

        // cargo run -- run --host http://localhost:6610 --key $PROJECT_KEY --project the-computer --branch feature-branch --branch-start-point master --testbed base --quiet bencher mock
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            project_key.as_str(),
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
            "--seed",
            PERFECT_SEED,
            "--measure",
            "latency",
            "--measure",
            MEASURE_SLUG,
        ])
        .env_remove(GITHUB_ACTIONS)
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- alert ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        std::thread::sleep(std::time::Duration::from_secs(1));

        // This shouldn't generate any alerts because we reset the thresholds on the start point branch
        // cargo run -- run --host http://localhost:6610 --key $BENCHER_API_KEY --project the-computer --branch feature-branch --branch-start-point master --testbed base --quiet bencher mock
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
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
            "--seed",
            PERFECT_SEED,
            "--pow",
            "10",
        ])
        .env_remove(GITHUB_ACTIONS)
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- alert ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        std::thread::sleep(std::time::Duration::from_secs(1));

        // This should generate alerts because we are using the measure that has a thresholds set for it on the start point branch
        // cargo run -- run --host http://localhost:6610 --key $BENCHER_API_KEY --project the-computer --branch feature-branch --branch-start-point master --testbed base --quiet bencher mock
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
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
            "--seed",
            PERFECT_SEED,
            "--measure",
            MEASURE_SLUG,
            "--pow",
            "10",
        ])
        .env_remove(GITHUB_ACTIONS)
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- alert ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 5);

        // cargo run -- report ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["report", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let reports: bencher_json::JsonReports =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        // The reports list collapses the results and alerts by default
        for report in &reports.0 {
            assert!(
                report.results.is_none(),
                "Report results should be collapsed by default"
            );
            assert!(
                report.alerts.is_none(),
                "Report alerts should be collapsed by default"
            );
            assert!(
                !report.counts.results.is_empty(),
                "Report counts should always be included"
            );
        }
        // The reports are in reverse chronological order,
        // so the first report is the one that generated the alerts
        let first_report = reports.0.first().unwrap();
        assert_eq!(first_report.counts.alerts.total, 5);
        assert_eq!(first_report.counts.alerts.active, 5);

        // cargo run -- report ls --host http://localhost:6610 --expand the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["report", "ls", HOST_ARG, host, "--expand", PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let reports: bencher_json::JsonReports =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        // Expanded reports include the full results and alerts,
        // and the counts are consistent with them
        for report in &reports.0 {
            let results = report.results.as_ref().unwrap();
            assert_eq!(results.len(), report.counts.results.len());
            let alerts = report.alerts.as_ref().unwrap();
            assert_eq!(
                u32::try_from(alerts.len()).unwrap(),
                report.counts.alerts.total
            );
        }
        let first_report = reports.0.first().unwrap();
        assert_eq!(first_report.counts.alerts.total, 5);

        std::thread::sleep(std::time::Duration::from_secs(1));

        // Reset the feature branch
        // cargo run -- run --host http://localhost:6610 --key $BENCHER_API_KEY --project the-computer --branch master --branch-reset --testbed base --quiet bencher mock
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
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
            "--seed",
            PERFECT_SEED,
        ])
        .env_remove(GITHUB_ACTIONS)
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- alert ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 5);

        // These alerts should be silenced
        // cargo run -- alert ls --host http://localhost:6610 the-computer --status active
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

        // cargo run -- alert ls --host http://localhost:6610 the-computer --status silenced
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
        // cargo run -- archive --host http://localhost:6610 --key $BENCHER_API_KEY --project the-computer --branch feature-branch
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "archive",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
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

        // cargo run -- alert ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 0);

        // These alerts should be silenced
        // cargo run -- alert ls --host http://localhost:6610 the-computer --status active
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

        // cargo run -- alert ls --host http://localhost:6610 the-computer --status silenced
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

        // cargo run -- alert ls --host http://localhost:6610 the-computer --status silenced
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
        // cargo run -- archive --host http://localhost:6610 --key $BENCHER_API_KEY --project the-computer --branch feature-branch
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "unarchive",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
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

        // cargo run -- alert ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 5);

        // These alerts should be silenced
        // cargo run -- alert ls --host http://localhost:6610 the-computer --status active
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

        // cargo run -- alert ls --host http://localhost:6610 the-computer --status silenced
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

        // cargo run -- alert ls --host http://localhost:6610 the-computer --status silenced
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

        // cargo run -- alert ls --host http://localhost:6610 the-computer
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["alert", "ls", HOST_ARG, host, PROJECT_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let alerts: bencher_json::JsonAlerts =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(alerts.0.len(), 5);

        std::thread::sleep(std::time::Duration::from_secs(1));

        // Generate alerts on the master branch using already defined Thresholds
        // cargo run -- run --host http://localhost:6610 --key $BENCHER_API_KEY --project the-computer --branch master --testbed base --quiet bencher mock --pow 10
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
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
            "--seed",
            PERFECT_SEED,
            "--measure",
            MEASURE_SLUG,
            "--pow",
            "10",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // cargo run -- alert ls --host http://localhost:6610 the-computer
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

        // cargo run -- alert ls --host http://localhost:6610 the-computer --status active
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

        // cargo run -- alert ls --host http://localhost:6610 the-computer --status silenced
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

        // cargo run -- alert get --host http://localhost:6610 --key $BENCHER_API_KEY the-computer [alert.uuid]
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "alert",
            "view",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            PROJECT_SLUG,
            #[expect(clippy::indexing_slicing, reason = "test asserts alerts exist above")]
            alerts.0[0].uuid.to_string().as_str(),
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _alert: bencher_json::JsonAlert =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        std::thread::sleep(std::time::Duration::from_secs(1));

        // If the start point hash is missing, fall back to the start point branch's latest version
        // https://github.com/bencherdev/bencher/issues/450
        // https://github.com/bencherdev/bencher/issues/774
        // cargo run -- run --host http://localhost:6610 --key $BENCHER_API_KEY --project the-computer --branch feature-hash --branch-start-point master --branch-start-point-hash badbadbadbadbadbadbadbadbadbadbadbadbad1 --testbed base --quiet bencher mock
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
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
            "--seed",
            PERFECT_SEED,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        // The hash doesn't exist, so it falls back to the start point branch's latest version
        assert!(json.branch.head.start_point.is_some(), "{json:?}");

        std::thread::sleep(std::time::Duration::from_secs(1));

        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
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
            "--seed",
            PERFECT_SEED,
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

        // If the start point hash does not exist, fall back to the start point branch's latest version
        // https://github.com/bencherdev/bencher/issues/774
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            PROJECT_ARG,
            PROJECT_SLUG,
            BRANCH_ARG,
            "feature-hash",
            HASH_ARG,
            &hash.next(),
            "--start-point",
            BRANCH_SLUG,
            "--start-point-hash",
            "badbadbadbadbadbadbadbadbadbadbadbadbad2",
            TESTBED_ARG,
            TESTBED_SLUG,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--seed",
            PERFECT_SEED,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        // The hash doesn't exist, so it falls back to the start point branch's latest version
        assert!(json.branch.head.start_point.is_some(), "{json:?}");

        std::thread::sleep(std::time::Duration::from_secs(1));

        // If the start point branch does not exist, fall back to no start point
        // https://github.com/bencherdev/bencher/issues/774
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            PROJECT_ARG,
            PROJECT_SLUG,
            BRANCH_ARG,
            "feature-hash-no-sp",
            HASH_ARG,
            &hash.next(),
            "--start-point",
            "non-existent-branch",
            "--start-point-hash",
            "badbadbadbadbadbadbadbadbadbadbadbadbad3",
            TESTBED_ARG,
            TESTBED_SLUG,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--seed",
            PERFECT_SEED,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        // The start point branch doesn't exist, so there is no start point
        assert!(json.branch.head.start_point.is_none(), "{json:?}");

        std::thread::sleep(std::time::Duration::from_secs(1));

        // Anonymous report
        // It should use the same on-the-fly project across multiple runs
        let mut anonymous_project: Option<bencher_json::JsonProject> = None;
        for _ in 0..3 {
            let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
            cmd.args([
                "run",
                HOST_ARG,
                host,
                // This is needed to run in CI
                "--ci-on-the-fly",
                "--format",
                "json",
                "--quiet",
                &bencher_cmd,
                "mock",
                "--seed",
                PERFECT_SEED,
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
                assert_eq!(
                    json.project.uuid.as_ref(),
                    json.project.organization.as_ref()
                );
                assert_eq!(json.project.name.as_ref(), self.project_name());
                if self.no_git {
                    // Without git, there is no repo name or hash in the slug
                    // The slug is just the testbed fingerprint
                    assert_eq!(json.project.slug.to_string().len(), 13, "{json:?}");
                } else {
                    assert!(
                        json.project.slug.to_string().starts_with(REPO_NAME),
                        "{json:?}"
                    );
                    assert_eq!(
                        json.project.slug.to_string().len(),
                        REPO_NAME.len() + 1 + 7 + 1 + 13,
                        "{json:?}"
                    );
                }
                assert_eq!(json.project.claimed, None);
                anonymous_project.replace(json.project);
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(1));

        // Anonymous report with project slug
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
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
            "--seed",
            PERFECT_SEED,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(json.project.name.as_ref(), self.project_name());
        assert_eq!(json.project.slug.to_string(), UNCLAIMED_SLUG);
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
            KEY_ARG,
            key,
            &organization_uuid_str,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonOrganization =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(json.uuid, organization_uuid);
        assert_eq!(json.name.as_ref(), self.project_name());
        assert_eq!(json.slug.to_string(), UNCLAIMED_SLUG);
        assert!(json.claimed.is_some(), "{json:?}");

        std::thread::sleep(std::time::Duration::from_secs(1));

        // Claimed report with project slug
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            PROJECT_ARG,
            UNCLAIMED_SLUG,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--seed",
            PERFECT_SEED,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(json.project.organization, organization_uuid);
        assert_eq!(json.project.name.as_ref(), self.project_name());
        assert_eq!(json.project.slug.to_string(), UNCLAIMED_SLUG);
        assert!(json.project.claimed.is_some(), "{json:?}");

        std::thread::sleep(std::time::Duration::from_secs(1));

        // On-the-fly project for user
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            // This is needed to run in CI
            "--ci-on-the-fly",
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--seed",
            PERFECT_SEED,
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
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            PROJECT_ARG,
            CLAIMED_SLUG,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--seed",
            PERFECT_SEED,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(json.project.organization, muriel_bagge_org_uuid);
        assert_eq!(json.project.name.as_ref(), self.project_name());
        assert_eq!(json.project.slug.to_string(), CLAIMED_SLUG);
        assert!(json.project.claimed.is_some(), "{json:?}");

        std::thread::sleep(std::time::Duration::from_secs(1));

        // Another on-the-fly project for user with project slug
        let bencher_one = "bencher-one";
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            PROJECT_ARG,
            bencher_one,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--seed",
            PERFECT_SEED,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(json.project.organization, muriel_bagge_org_uuid);
        let project_name = self.project_name();
        assert_eq!(json.project.name.as_ref(), format!("{project_name} (1)"));
        assert_eq!(json.project.slug.to_string(), bencher_one);
        assert!(json.project.claimed.is_some(), "{json:?}");

        std::thread::sleep(std::time::Duration::from_secs(1));

        // Yet another on-the-fly project for user with project slug
        let bencher_two = "bencher-two";
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            PROJECT_ARG,
            bencher_two,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--seed",
            PERFECT_SEED,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(json.project.organization, muriel_bagge_org_uuid);
        assert_eq!(json.project.name.as_ref(), format!("{project_name} (2)"));
        assert_eq!(json.project.slug.to_string(), bencher_two);
        assert!(json.project.claimed.is_some(), "{json:?}");

        // ========== Project Key CRUD Lifecycle ==========
        // Only a browser session lists, views, renames, or revokes a key.
        let session = &readers.user.session;
        let project_keys_path = format!("/v0/projects/{PROJECT_SLUG}/keys");
        let project_key_path = format!("{project_keys_path}/{project_key_uuid_str}");

        let keys: bencher_json::JsonProjectKeys =
            serde_json::from_str(&session.get(&project_keys_path)?)?;
        assert_eq!(
            keys.0.len(),
            1,
            "should have exactly one active key: {keys:?}"
        );
        #[expect(
            clippy::indexing_slicing,
            reason = "test asserts exactly one key above"
        )]
        {
            assert_eq!(keys.0[0].uuid, project_key_created.uuid);
            assert!(
                keys.0[0].revoked.is_none(),
                "key should not be revoked: {keys:?}"
            );
        }

        let key_view: bencher_json::JsonProjectKey =
            serde_json::from_str(&session.get(&project_key_path)?)?;
        assert_eq!(key_view.uuid, project_key_created.uuid);
        assert_eq!(key_view.name.as_ref(), PROJECT_KEY_NAME);
        assert!(
            key_view.revoked.is_none(),
            "key should not be revoked: {key_view:?}"
        );

        let key_updated: bencher_json::JsonProjectKey = serde_json::from_str(&session.send_json(
            "PATCH",
            &project_key_path,
            &serde_json::json!({ "name": PROJECT_KEY_UPDATED_NAME }),
        )?)?;
        assert_eq!(key_updated.uuid, project_key_created.uuid);
        assert_eq!(key_updated.name.as_ref(), PROJECT_KEY_UPDATED_NAME);

        // Verify the key still works after rename
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            project_key.as_str(),
            PROJECT_ARG,
            PROJECT_SLUG,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--seed",
            PERFECT_SEED,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        assert_eq!(session.delete(&project_key_path)?, 204);

        // The revoked key must NOT authenticate a bencher run
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            project_key.as_str(),
            PROJECT_ARG,
            PROJECT_SLUG,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--seed",
            PERFECT_SEED,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().failure();
        let output = assert.get_output();
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Status: 401"),
            "revoked project key should 401: {output:?}"
        );

        // Default list must hide the revoked key
        let keys: bencher_json::JsonProjectKeys =
            serde_json::from_str(&session.get(&project_keys_path)?)?;
        assert!(
            keys.0.iter().all(|k| k.uuid != project_key_created.uuid),
            "default list should hide revoked keys: {keys:?}"
        );

        // `revoked=true` must surface the revoked key
        let keys: bencher_json::JsonProjectKeys =
            serde_json::from_str(&session.get(&format!("{project_keys_path}?revoked=true"))?)?;
        let revoked_entry = keys
            .0
            .iter()
            .find(|k| k.uuid == project_key_created.uuid)
            .expect("revoked key should appear when revoked keys are asked for");
        assert!(
            revoked_entry.revoked.is_some(),
            "revoked key must carry a revoked timestamp: {revoked_entry:?}"
        );

        // Revoking the same key again should fail (already revoked)
        assert_eq!(
            session.delete(&project_key_path)?,
            409,
            "a revoked project key should not be revoked again"
        );

        // A project key without `--project` must fail fast in the CLI,
        // before any request is made (so the revoked state above is irrelevant).
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            project_key.as_str(),
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--seed",
            PERFECT_SEED,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().failure();
        let output = assert.get_output();
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("requires `--project`"),
            "project key without --project should fail fast: {output:?}"
        );

        // ========== User Key CRUD Lifecycle ==========

        let user_key_created = session.create_user_key(ORG_SLUG, USER_KEY_NAME)?;
        let user_key = user_key_created.key.as_ref().to_owned();
        let user_key_uuid_str = user_key_created.uuid.to_string();
        let user_keys_path = format!("/v0/users/{ORG_SLUG}/keys");
        let user_key_path = format!("{user_keys_path}/{user_key_uuid_str}");

        // cargo run -- run --host http://localhost:6610 --key $USER_KEY --project the-computer --quiet bencher mock --seed 6
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            user_key.as_str(),
            PROJECT_ARG,
            PROJECT_SLUG,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--seed",
            PERFECT_SEED,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // A user key is a drop-in JWT replacement on user endpoints too:
        // cargo run -- user view --host http://localhost:6610 --key $USER_KEY muriel-bagge
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "user",
            "view",
            HOST_ARG,
            host,
            KEY_ARG,
            user_key.as_str(),
            ORG_SLUG,
        ])
        .current_dir(CLI_DIR);
        cmd.assert().success();

        // A user key must NOT be able to create another user key
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "user",
            "key",
            "create",
            HOST_ARG,
            host,
            KEY_ARG,
            user_key.as_str(),
            "--name",
            "minted-by-key",
            ORG_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().failure();
        let output = assert.get_output();
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Status: 403"),
            "user key creating a user key should 403: {output:?}"
        );

        // Muriel's own key, which the seed's requests send, and the new one.
        let keys: bencher_json::JsonUserKeys =
            serde_json::from_str(&session.get(&user_keys_path)?)?;
        assert_eq!(
            keys.0.len(),
            2,
            "should have two active user keys: {keys:?}"
        );
        let listed = keys
            .0
            .iter()
            .find(|k| k.uuid == user_key_created.uuid)
            .expect("the new user key should be listed");
        assert!(
            listed.revoked.is_none(),
            "key should not be revoked: {keys:?}"
        );

        let key_view: bencher_json::JsonUserKey =
            serde_json::from_str(&session.get(&user_key_path)?)?;
        assert_eq!(key_view.uuid, user_key_created.uuid);
        assert_eq!(key_view.name.as_ref(), USER_KEY_NAME);
        assert!(
            key_view.revoked.is_none(),
            "key should not be revoked: {key_view:?}"
        );

        let key_updated: bencher_json::JsonUserKey = serde_json::from_str(&session.send_json(
            "PATCH",
            &user_key_path,
            &serde_json::json!({ "name": USER_KEY_UPDATED_NAME }),
        )?)?;
        assert_eq!(key_updated.uuid, user_key_created.uuid);
        assert_eq!(key_updated.name.as_ref(), USER_KEY_UPDATED_NAME);

        // Verify the key still works after rename
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            user_key.as_str(),
            PROJECT_ARG,
            PROJECT_SLUG,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--seed",
            PERFECT_SEED,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonReport =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // A user key cannot revoke itself: only a user session manages keys.
        // cargo run -- user key revoke --host http://localhost:6610 --key $USER_KEY muriel-bagge <uuid>
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "user",
            "key",
            "revoke",
            HOST_ARG,
            host,
            KEY_ARG,
            user_key.as_str(),
            ORG_SLUG,
            user_key_uuid_str.as_str(),
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().failure();
        let output = assert.get_output();
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Status: 403"),
            "user key revoking itself should 403: {output:?}"
        );

        assert_eq!(session.delete(&user_key_path)?, 204);

        // The revoked key must NOT authenticate a bencher run
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            user_key.as_str(),
            PROJECT_ARG,
            PROJECT_SLUG,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--seed",
            PERFECT_SEED,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().failure();
        let output = assert.get_output();
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Status: 401"),
            "revoked user key should 401: {output:?}"
        );

        // Default list must hide the revoked key
        let keys: bencher_json::JsonUserKeys =
            serde_json::from_str(&session.get(&user_keys_path)?)?;
        assert!(
            keys.0.iter().all(|k| k.uuid != user_key_created.uuid),
            "default list should hide revoked keys: {keys:?}"
        );

        // `revoked=true` must surface the revoked key
        let keys: bencher_json::JsonUserKeys =
            serde_json::from_str(&session.get(&format!("{user_keys_path}?revoked=true"))?)?;
        let revoked_entry = keys
            .0
            .iter()
            .find(|k| k.uuid == user_key_created.uuid)
            .expect("revoked key should appear when revoked keys are asked for");
        assert!(
            revoked_entry.revoked.is_some(),
            "revoked key must carry a revoked timestamp: {revoked_entry:?}"
        );

        // Revoking the same key again should fail (already revoked)
        assert_eq!(
            session.delete(&user_key_path)?,
            409,
            "a revoked user key should not be revoked again"
        );

        #[cfg(feature = "plus")]
        self.plus_exec(readers)?;

        Ok(())
    }

    #[cfg(feature = "plus")]
    fn plus_exec(&self, readers: &Readers) -> anyhow::Result<()> {
        if self.is_bencher_cloud {
            self.bencher_cloud_exec(readers)?;
        } else {
            self.bencher_self_hosted_exec(readers)?;
        }

        let host = self.url.as_ref();
        let admin_key = readers.admin.key.as_ref();

        // cargo run -- server stats --host http://localhost:6610 --key $ADMIN_BENCHER_API_KEY
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["server", "stats", HOST_ARG, host, KEY_ARG, admin_key])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _json: bencher_json::JsonServerStats =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        self.runner_spec_exec(readers)?;

        Ok(())
    }

    #[cfg(feature = "plus")]
    #[expect(
        clippy::panic_in_result_fn,
        clippy::too_many_lines,
        reason = "integration test exercises full spec workflow"
    )]
    fn runner_spec_exec(&self, readers: &Readers) -> anyhow::Result<()> {
        let host = self.url.as_ref();
        let admin_key = readers.admin.key.as_ref();

        // Create a spec
        // cargo run -- spec create --host http://localhost:6610 --key $ADMIN_BENCHER_API_KEY --name "Test Spec" --architecture x86_64 --sandbox firecracker --cpu 4 --memory 8589934592 --disk 1073741824
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "spec",
            "create",
            HOST_ARG,
            host,
            KEY_ARG,
            admin_key,
            "--name",
            "Test Spec",
            "--os",
            "linux",
            "--architecture",
            "x86_64",
            "--sandbox",
            "firecracker",
            "--cpu",
            "4",
            "--memory",
            "8589934592",
            "--disk",
            "1073741824",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let spec: bencher_json::JsonSpec =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(spec.name.as_ref(), "Test Spec");
        assert_eq!(AsRef::<str>::as_ref(&spec.slug), "test-spec");
        let spec_uuid = spec.uuid;

        // List specs
        // cargo run -- spec list --host http://localhost:6610 --key $ADMIN_BENCHER_API_KEY
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["spec", "list", HOST_ARG, host, KEY_ARG, admin_key])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let specs: bencher_json::JsonSpecs =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(specs.0.len(), 1);

        // View spec
        // cargo run -- spec view --host http://localhost:6610 --key $ADMIN_BENCHER_API_KEY test-spec
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "spec",
            "view",
            HOST_ARG,
            host,
            KEY_ARG,
            admin_key,
            "test-spec",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let spec: bencher_json::JsonSpec =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(spec.uuid, spec_uuid);

        // Create a runner
        // cargo run -- runner create --host http://localhost:6610 --key $ADMIN_BENCHER_API_KEY --name "Test Runner"
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "runner",
            "create",
            HOST_ARG,
            host,
            KEY_ARG,
            admin_key,
            "--name",
            "Test Runner",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let runner_key: bencher_json::JsonRunnerKey =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        let runner_uuid = runner_key.uuid;

        // List runners
        // cargo run -- runner list --host http://localhost:6610 --key $ADMIN_BENCHER_API_KEY
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["runner", "list", HOST_ARG, host, KEY_ARG, admin_key])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let runners: bencher_json::JsonRunners =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(runners.0.len(), 1);

        // View runner (no specs assigned yet)
        // cargo run -- runner view --host http://localhost:6610 --key $ADMIN_BENCHER_API_KEY test-runner
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "runner",
            "view",
            HOST_ARG,
            host,
            KEY_ARG,
            admin_key,
            "test-runner",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let runner: bencher_json::JsonRunner =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(runner.uuid, runner_uuid);
        assert_eq!(runner.name.as_ref(), "Test Runner");
        assert!(runner.specs.is_empty(), "Expected no specs for new runner");

        // Add spec to runner
        // cargo run -- runner spec add --host http://localhost:6610 --key $ADMIN_BENCHER_API_KEY --spec test-spec test-runner
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "runner",
            "spec",
            "add",
            HOST_ARG,
            host,
            KEY_ARG,
            admin_key,
            "--spec",
            "test-spec",
            "test-runner",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let runner: bencher_json::JsonRunner =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(runner.uuid, runner_uuid);
        assert!(
            runner.specs.contains(&spec_uuid),
            "Expected spec to be assigned to runner"
        );

        // View runner again (spec should now be assigned)
        // cargo run -- runner view --host http://localhost:6610 --key $ADMIN_BENCHER_API_KEY test-runner
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "runner",
            "view",
            HOST_ARG,
            host,
            KEY_ARG,
            admin_key,
            "test-runner",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let runner: bencher_json::JsonRunner =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(runner.specs.len(), 1, "Expected one spec for runner");
        assert_eq!(
            runner.specs.first().copied(),
            Some(spec_uuid),
            "Expected spec UUID to match"
        );

        // Create a non-sandboxed spec
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "spec",
            "create",
            HOST_ARG,
            host,
            KEY_ARG,
            admin_key,
            "--name",
            "No Sandbox Spec",
            "--os",
            "linux",
            "--architecture",
            "x86_64",
            "--cpu",
            "4",
            "--memory",
            "8589934592",
            "--disk",
            "1073741824",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _nosandbox_spec: bencher_json::JsonSpec =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // Create a second runner for non-sandboxed execution
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "runner",
            "create",
            HOST_ARG,
            host,
            KEY_ARG,
            admin_key,
            "--name",
            "Test Runner No Sandbox",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let _nosandbox_runner_key: bencher_json::JsonRunnerKey =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();

        // Add spec to no-sandbox runner
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "runner",
            "spec",
            "add",
            HOST_ARG,
            host,
            KEY_ARG,
            admin_key,
            "--spec",
            "no-sandbox-spec",
            "test-runner-no-sandbox",
        ])
        .current_dir(CLI_DIR);
        cmd.assert().success();

        // Rotate runner key
        // cargo run -- runner key --host http://localhost:6610 --key $ADMIN_BENCHER_API_KEY test-runner
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "runner",
            "key",
            HOST_ARG,
            host,
            KEY_ARG,
            admin_key,
            "test-runner",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let new_key: bencher_json::JsonRunnerKey =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(new_key.uuid, runner_uuid);

        // Update runner name
        // cargo run -- runner update --host http://localhost:6610 --key $ADMIN_BENCHER_API_KEY --name "Updated Runner" test-runner
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "runner",
            "update",
            HOST_ARG,
            host,
            KEY_ARG,
            admin_key,
            "--name",
            "Updated Runner",
            "test-runner",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let runner: bencher_json::JsonRunner =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(runner.name.as_ref(), "Updated Runner");

        // Update spec name
        // cargo run -- spec update --host http://localhost:6610 --key $ADMIN_BENCHER_API_KEY --name "Updated Spec" test-spec
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "spec",
            "update",
            HOST_ARG,
            host,
            KEY_ARG,
            admin_key,
            "--name",
            "Updated Spec",
            "test-spec",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let spec: bencher_json::JsonSpec =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(spec.name.as_ref(), "Updated Spec");

        // Assign spec to testbed via testbed update
        // cargo run -- testbed update --host http://localhost:6610 --key $BENCHER_API_KEY --spec test-spec the-computer base
        let key = readers.user.key.as_ref();
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "testbed",
            "update",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            "--spec",
            "test-spec",
            PROJECT_SLUG,
            "base",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let testbed: bencher_json::JsonTestbed =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert!(
            testbed.spec.is_some(),
            "Expected spec on testbed after assigning"
        );

        // Run with --spec-reset to clear the spec
        // cargo run -- run --host http://localhost:6610 --key $BENCHER_API_KEY --project the-computer --testbed base --spec-reset --quiet bencher mock
        let bencher_cmd = Command::cargo_bin(BENCHER_CMD)?
            .get_program()
            .to_string_lossy()
            .to_string();
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            PROJECT_ARG,
            PROJECT_SLUG,
            "--testbed",
            "base",
            "--spec-reset",
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

        // Verify testbed spec is cleared
        // cargo run -- testbed view --host http://localhost:6610 --key $BENCHER_API_KEY the-computer base
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "testbed",
            "view",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            PROJECT_SLUG,
            "base",
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let testbed: bencher_json::JsonTestbed =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert!(
            testbed.spec.is_none(),
            "Expected spec to be cleared after --spec-reset"
        );

        Ok(())
    }

    #[cfg(feature = "plus")]
    #[expect(
        clippy::panic_in_result_fn,
        clippy::too_many_lines,
        reason = "integration test exercises full cloud workflow"
    )]
    fn bencher_cloud_exec(&self, readers: &Readers) -> anyhow::Result<()> {
        let host = self.url.as_ref();
        let admin_key = readers.admin.key.as_ref();
        let key = readers.user.key.as_ref();
        let bencher_cmd = Command::cargo_bin(BENCHER_CMD)?
            .get_program()
            .to_string_lossy()
            .to_string();

        std::thread::sleep(std::time::Duration::from_secs(1));

        // Another unclaimed project
        // 5 metics x 51 runs = 255 metrics
        let unclaimed_max = "unclaimed-max";
        for _ in 0..51 {
            let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
            cmd.args([
                "run",
                HOST_ARG,
                host,
                PROJECT_ARG,
                unclaimed_max,
                "--format",
                "json",
                "--quiet",
                &bencher_cmd,
                "mock",
                "--seed",
                PERFECT_SEED,
            ])
            .current_dir(CLI_DIR);
            let assert = cmd.assert().success();
            let json: bencher_json::JsonReport =
                serde_json::from_slice(&assert.get_output().stdout).unwrap();
            assert_eq!(json.project.slug.to_string(), unclaimed_max);
            assert!(json.project.claimed.is_none(), "{json:?}");
        }

        std::thread::sleep(std::time::Duration::from_secs(1));

        // This run should fail as we hit the unclaimed rate limit
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "run",
            HOST_ARG,
            host,
            PROJECT_ARG,
            unclaimed_max,
            "--format",
            "json",
            "--quiet",
            &bencher_cmd,
            "mock",
            "--seed",
            PERFECT_SEED,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().failure();
        let output = assert.get_output();
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Status: 429 Too Many Requests"),
            "{output:?}"
        );

        // An address at its cap of sign-in emails still lands on Check your inbox, with the wait,
        // and no email goes out.
        self.sign_in_email_caps("courage@nowhere.com")?;

        // cargo run -- sso list --host http://localhost:6610 --key $BENCHER_API_KEY muriel-bagge
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["sso", "list", HOST_ARG, host, KEY_ARG, key, ORG_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonSsos =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(json.0.len(), 0);

        // cargo run -- sso create --host http://localhost:6610 --key $ADMIN_BENCHER_API_KEY --domain nowhere.com muriel-bagge
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "sso",
            "create",
            HOST_ARG,
            host,
            KEY_ARG,
            admin_key,
            "--domain",
            "nowhere.com",
            ORG_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonSso =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        let sso_uuid = json.uuid;

        // cargo run -- sso list --host http://localhost:6610 --key $BENCHER_API_KEY muriel-bagge
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["sso", "list", HOST_ARG, host, KEY_ARG, key, ORG_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonSsos =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(json.0.len(), 1);

        // cargo run -- sso view --host http://localhost:6610 --key $BENCHER_API_KEY muriel-bagge [sso_uuid]
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "sso",
            "view",
            HOST_ARG,
            host,
            KEY_ARG,
            key,
            ORG_SLUG,
            &sso_uuid.to_string(),
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonSso =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(json.uuid, sso_uuid);

        Ok(())
    }

    /// Start sign-ins for `email` back to back until the address is at its cap, which must be its
    /// cap for the minute, since that is the lowest: the start at the cap answers with the minute's
    /// wait, and only the starts before it email the address.
    #[cfg(feature = "plus")]
    fn sign_in_email_caps(&self, email: &str) -> anyhow::Result<()> {
        /// More sign-ins than the default daily cap of an address allows.
        const ATTEMPTS: usize = 13;
        /// The longest wait the cap for the minute gives, in seconds: an email counts for the
        /// minute and at most a twelfth of a minute longer.
        const MINUTE_CAP_WAIT: u32 = 65;
        /// An address emailed after the start at the cap, so that once its email is in the log, so
        /// is any email that start sent.
        const AFTER_THE_CAP: &str = "shirley@nowhere.com";
        let mark = self.api_log.mark()?;
        let mut sent = 0;
        let wait = loop {
            anyhow::ensure!(
                sent < ATTEMPTS,
                "{email} never reached its cap of sign-in emails in {ATTEMPTS} sign-ins"
            );
            let started = BrowserSession::start_sign_in(&self.url, email).map_err(|e| {
                anyhow::anyhow!(
                    "Sign-in {sent} for {email} was refused before the address reached its cap: \
                     {e}"
                )
            })?;
            match started.wait {
                Some(wait) => break wait,
                None => sent += 1,
            }
        };
        anyhow::ensure!(sent > 0, "{email} was at a cap before any email");
        anyhow::ensure!(
            (1..=MINUTE_CAP_WAIT).contains(&wait),
            "{email} reached a cap other than the minute's after {sent} emails: it waits {wait} \
             seconds"
        );
        let after = BrowserSession::start_sign_in(&self.url, AFTER_THE_CAP)?;
        anyhow::ensure!(after.wait.is_none(), "{AFTER_THE_CAP} was at its cap");
        self.api_log.sign_in_code(&mark, AFTER_THE_CAP)?;
        let emailed = self.api_log.sign_in_emails(&mark, email)?;
        anyhow::ensure!(
            emailed == sent,
            "{email} got {emailed} sign-in emails from {sent} sign-ins before its cap and 1 at it"
        );
        Ok(())
    }

    #[cfg(feature = "plus")]
    #[expect(
        clippy::panic_in_result_fn,
        reason = "integration test exercises full self-hosted workflow"
    )]
    fn bencher_self_hosted_exec(&self, readers: &Readers) -> anyhow::Result<()> {
        let host = self.url.as_ref();
        let admin_key = readers.admin.key.as_ref();
        let key = readers.user.key.as_ref();

        // cargo run -- sso list --host http://localhost:6610 --key $BENCHER_API_KEY muriel-bagge
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["sso", "list", HOST_ARG, host, KEY_ARG, key, ORG_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonSsos =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(json.0.len(), 0);

        // This should error for the self-hosted admin, since the organization doesn't have a valid license
        // cargo run -- sso create --host http://localhost:6610 --key $ADMIN_BENCHER_API_KEY --domain nowhere.com muriel-bagge
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args([
            "sso",
            "create",
            HOST_ARG,
            host,
            KEY_ARG,
            admin_key,
            "--domain",
            "nowhere.com",
            ORG_SLUG,
        ])
        .current_dir(CLI_DIR);
        let assert = cmd.assert().failure();
        let output = assert.get_output();
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Status: 402 Payment Required"),
            "{output:?}"
        );

        // cargo run -- sso list --host http://localhost:6610 --key $BENCHER_API_KEY muriel-bagge
        let mut cmd = Command::cargo_bin(BENCHER_CMD)?;
        cmd.args(["sso", "list", HOST_ARG, host, KEY_ARG, key, ORG_SLUG])
            .current_dir(CLI_DIR);
        let assert = cmd.assert().success();
        let json: bencher_json::JsonSsos =
            serde_json::from_slice(&assert.get_output().stdout).unwrap();
        assert_eq!(json.0.len(), 0);

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
