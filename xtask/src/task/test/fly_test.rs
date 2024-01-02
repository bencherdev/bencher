use std::process::Command;

use bencher_json::{JsonApiVersion, DEVEL_BENCHER_API_URL_STR, PROD_BENCHER_API_URL_STR};

use crate::{parser::CliFlyTest, task::types::swagger::swagger_spec};

const BENCHER_API_URL_KEY: &str = "BENCHER_API_URL";
const TEST_BENCHER_API_TOKEN: &str = "TEST_BENCHER_API_TOKEN";
const DEV_BENCHER_API_TOKEN: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo1OTkzNjQyMTU2LCJpYXQiOjE2OTg2NzQ4NjEsImlzcyI6Imh0dHBzOi8vZGV2ZWwtLWJlbmNoZXIubmV0bGlmeS5hcHAvIiwic3ViIjoibXVyaWVsLmJhZ2dlQG5vd2hlcmUuY29tIiwib3JnIjpudWxsfQ.9z7jmM53TcVzc1inDxTeX9_OR0PQPpZAsKsCE7lWHfo";

#[derive(Debug)]
pub struct FlyTest {
    pub dev: bool,
}

impl TryFrom<CliFlyTest> for FlyTest {
    type Error = anyhow::Error;

    fn try_from(swagger: CliFlyTest) -> Result<Self, Self::Error> {
        let CliFlyTest { dev } = swagger;
        Ok(Self { dev })
    }
}

impl FlyTest {
    pub fn exec(&self) -> anyhow::Result<()> {
        let swagger_spec = swagger_spec()?;
        let Some(version) = swagger_spec.version() else {
            return Err(anyhow::anyhow!("No version found in swagger.json"));
        };

        let api_url = if self.dev {
            DEVEL_BENCHER_API_URL_STR
        } else {
            PROD_BENCHER_API_URL_STR
        };
        test_api_version(api_url, version)?;

        if self.dev {
            seed(api_url)?;
        }

        Ok(())
    }
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
