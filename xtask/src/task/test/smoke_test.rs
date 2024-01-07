use std::process::Command;

use bencher_json::{
    JsonApiVersion, DEV_BENCHER_API_URL_STR, LOCALHOST_BENCHER_API_URL_STR,
    PROD_BENCHER_API_URL_STR, TEST_BENCHER_API_URL_STR,
};

use crate::{
    parser::{TaskSmokeTest, TaskTestEnvironment},
    task::types::swagger::swagger_spec,
};

const BENCHER_API_URL_KEY: &str = "BENCHER_API_URL";
const TEST_BENCHER_API_TOKEN: &str = "TEST_BENCHER_API_TOKEN";
const DEV_BENCHER_API_TOKEN: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo1OTkzNjQyMTU2LCJpYXQiOjE2OTg2NzQ4NjEsImlzcyI6Imh0dHBzOi8vZGV2ZWwtLWJlbmNoZXIubmV0bGlmeS5hcHAvIiwic3ViIjoibXVyaWVsLmJhZ2dlQG5vd2hlcmUuY29tIiwib3JnIjpudWxsfQ.9z7jmM53TcVzc1inDxTeX9_OR0PQPpZAsKsCE7lWHfo";

#[derive(Debug)]
pub struct SmokeTest {
    pub environment: Environment,
}

#[derive(Debug, Clone, Copy)]
pub enum Environment {
    Localhost,
    Docker,
    Dev,
    Test,
    Prod,
}

impl TryFrom<TaskSmokeTest> for SmokeTest {
    type Error = anyhow::Error;

    fn try_from(test: TaskSmokeTest) -> Result<Self, Self::Error> {
        let TaskSmokeTest { environment } = test;
        Ok(Self {
            environment: environment.into(),
        })
    }
}

impl From<TaskTestEnvironment> for Environment {
    fn from(endpoint: TaskTestEnvironment) -> Self {
        match endpoint {
            TaskTestEnvironment::Localhost => Self::Localhost,
            TaskTestEnvironment::Docker => Self::Docker,
            TaskTestEnvironment::Dev => Self::Dev,
            TaskTestEnvironment::Test => Self::Test,
            TaskTestEnvironment::Prod => Self::Prod,
        }
    }
}

impl SmokeTest {
    pub fn exec(&self) -> anyhow::Result<()> {
        let swagger_spec = swagger_spec()?;
        let Some(version) = swagger_spec.version() else {
            return Err(anyhow::anyhow!("No version found in swagger.json"));
        };

        match self.environment {
            Environment::Localhost => api_run()?,
            Environment::Docker => bencher_up()?,
            Environment::Dev | Environment::Test | Environment::Prod => {},
        }

        let api_url = self.environment.as_ref();
        test_api_version(api_url, version)?;
        if self.environment.should_seed() {
            seed(api_url)?;
            examples(api_url)?;
        }

        if let Environment::Docker = self.environment {
            bencher_down()?;
        }

        Ok(())
    }
}

impl AsRef<str> for Environment {
    fn as_ref(&self) -> &str {
        match self {
            Self::Localhost | Self::Docker => LOCALHOST_BENCHER_API_URL_STR,
            Self::Dev => DEV_BENCHER_API_URL_STR,
            Self::Test => TEST_BENCHER_API_URL_STR,
            Self::Prod => PROD_BENCHER_API_URL_STR,
        }
    }
}

impl Environment {
    pub fn should_seed(self) -> bool {
        matches!(self, Self::Localhost | Self::Dev)
    }
}

fn api_run() -> anyhow::Result<()> {
    let _child = Command::new("cargo")
        .args(["run"])
        .current_dir("./services/api")
        .spawn()?;

    while std::net::TcpStream::connect("localhost:61016").is_err() {
        std::thread::sleep(std::time::Duration::from_secs(1));
        println!("Waiting for API server to start...");
    }

    Ok(())
}

fn bencher_up() -> anyhow::Result<()> {
    let output = Command::new("cargo")
        .args(["run", "--", "up", "-d"])
        .current_dir("./services/cli")
        .output()?;

    output.status.success().then_some(()).ok_or_else(|| {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        println!("{}", String::from_utf8_lossy(&output.stdout));

        anyhow::anyhow!(
            "Failed to run `bencher up`. Exit code: {:?}",
            output.status.code()
        )
    })?;

    while std::net::TcpStream::connect("localhost:61016").is_err() {
        std::thread::sleep(std::time::Duration::from_secs(1));
        println!("Waiting for API server to start...");
    }

    Ok(())
}

fn bencher_down() -> anyhow::Result<()> {
    let output = Command::new("cargo")
        .args(["run", "--", "down"])
        .current_dir("./services/cli")
        .output()?;

    output.status.success().then_some(()).ok_or_else(|| {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        println!("{}", String::from_utf8_lossy(&output.stdout));

        anyhow::anyhow!(
            "Failed to run `bencher down`. Exit code: {:?}",
            output.status.code()
        )
    })?;

    Ok(())
}

fn test_api_version(api_url: &str, version: &str) -> anyhow::Result<()> {
    println!("Testing API deploy is version {version} at {api_url}");

    let output = Command::new("cargo")
        .args(["run", "--", "server", "version", "--host", api_url])
        .current_dir("./services/cli")
        .output()?;

    output.status.success().then_some(()).ok_or_else(|| {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        println!("{}", String::from_utf8_lossy(&output.stdout));

        anyhow::anyhow!(
            "Failed to get server version. Exit code: {:?}",
            output.status.code()
        )
    })?;

    let api_version =
        serde_json::from_str::<JsonApiVersion>(std::str::from_utf8(&output.stdout)?)?.version;
    if api_version != version {
        return Err(anyhow::anyhow!(
            "API version {api_version} does not match swagger.json version {version}"
        ));
    }

    Ok(())
}

fn seed(api_url: &str) -> anyhow::Result<()> {
    println!("Seeding API deploy at {api_url}");

    let output = Command::new("cargo")
        .args(["test", "--features", "seed", "--test", "seed"])
        .current_dir("./services/cli")
        .env(BENCHER_API_URL_KEY, api_url)
        .env(TEST_BENCHER_API_TOKEN, DEV_BENCHER_API_TOKEN)
        .output()?;

    output.status.success().then_some(()).ok_or_else(|| {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        println!("{}", String::from_utf8_lossy(&output.stdout));

        anyhow::anyhow!("Failed to seed. Exit code: {:?}", output.status.code())
    })
}

fn examples(api_url: &str) -> anyhow::Result<()> {
    println!("Running examples at {api_url}");

    let output = Command::new("bencher")
        .args([
            "run",
            "--host",
            api_url,
            "--token",
            DEV_BENCHER_API_TOKEN,
            "--project",
            "the-computer",
            "--branch",
            "master",
            "--testbed",
            "base",
            "cargo bench",
        ])
        .current_dir("./examples/rust/bench")
        .output()?;

    output.status.success().then_some(()).ok_or_else(|| {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        println!("{}", String::from_utf8_lossy(&output.stdout));

        anyhow::anyhow!(
            "Failed to run examples. Exit code: {:?}",
            output.status.code()
        )
    })
}
