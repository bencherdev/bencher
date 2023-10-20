use std::{
    fs::File,
    io::{BufRead, BufReader},
    process::Command,
};

use bencher_json::JsonApiVersion;
use camino::Utf8PathBuf;

use crate::{parser::CliNetlifyTest, task::swagger::swagger_spec};

const NETLIFY_URL: &str = "https://app.netlify.com/sites/bencher/deploys/";
const BENCHER_API_URL_KEY: &str = "BENCHER_API_URL";
const DEV_BENCHER_API_URL: &str = "https://bencher-api-dev.fly.dev/";

#[derive(Debug)]
pub struct NetlifyTest {}

impl TryFrom<CliNetlifyTest> for NetlifyTest {
    type Error = anyhow::Error;

    fn try_from(_swagger: CliNetlifyTest) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl NetlifyTest {
    pub async fn exec(&self) -> anyhow::Result<()> {
        let netlify_path = Utf8PathBuf::from("netlify.txt");
        let netlify_file = File::open(netlify_path)?;

        let mut deploy_id = None;
        let buffered_reader = BufReader::new(netlify_file);
        // Looking for a line like:
        // Build logs:        https://app.netlify.com/sites/bencher/deploys/65324dc5185e4f0b9e4a6e25
        for line in buffered_reader.lines() {
            let line = line?;
            let Some((_, url)) = line.split_once("Build logs:") else {
                continue;
            };
            let Some((_, id)) = url.trim().split_once(NETLIFY_URL) else {
                return Err(anyhow::anyhow!(
                    "Netlify URL {url} does not match {NETLIFY_URL}"
                ));
            };
            println!("Netlify Deploy ID: {id}");
            deploy_id = Some(id.to_owned());
            break;
        }
        let Some(deploy_id) = deploy_id else {
            return Err(anyhow::anyhow!("No Netlify Deploy ID found"));
        };

        let swagger_spec = swagger_spec()?;
        let Some(version) = swagger_spec.version() else {
            return Err(anyhow::anyhow!("No version found in swagger.json"));
        };

        let console_url = format!("https://{deploy_id}--bencher.netlify.app");
        println!("Testing Netlify Deploy: {console_url}");
        let html = reqwest::get(console_url).await?.text().await?;

        // Looking for a line like:
        // BETA v<!--#-->0.3.13<!--/-->
        for line in html.lines() {
            let Some((_, v)) = line.split_once("BETA v<!--#-->") else {
                continue;
            };
            let Some((console_version, _)) = v.split_once("<!--/-->") else {
                return Err(anyhow::anyhow!(
                    "Console version {v} does not match expected format"
                ));
            };
            if console_version != version {
                return Err(anyhow::anyhow!(
                    "Console version {console_version} does not match swagger.json version {version}"
                ));
            }
        }

        println!("Testing Fly.io Deploy: {DEV_BENCHER_API_URL}");
        let output = Command::new("cargo")
            .args([
                "run",
                "--",
                "server",
                "version",
                "--host",
                DEV_BENCHER_API_URL,
            ])
            .current_dir("./services/cli")
            .output()?;
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));

        println!("{}", String::from_utf8_lossy(&output.stdout));
        let api_version =
            serde_json::from_str::<JsonApiVersion>(std::str::from_utf8(&output.stdout)?)?.version;

        output.status.success().then_some(()).ok_or_else(|| {
            anyhow::anyhow!(
                "Failed to generate swagger.json. Exit code: {:?}",
                output.status.code()
            )
        })?;

        if api_version != version {
            return Err(anyhow::anyhow!(
                "API version {api_version} does not match swagger.json version {version}"
            ));
        }

        let output = Command::new("cargo")
            .args(["test", "--features", "seed", "--test", "seed"])
            .current_dir("./services/cli")
            .env(BENCHER_API_URL_KEY, DEV_BENCHER_API_URL)
            .output()?;
        println!("{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));

        output.status.success().then_some(()).ok_or_else(|| {
            anyhow::anyhow!("Failed to seed. Exit code: {:?}", output.status.code())
        })?;

        Ok(())
    }
}
