use std::fs::File;

use bencher_json::PROD_BENCHER_URL_STR;
use camino::Utf8PathBuf;

use crate::{parser::CliNetlifyTest, task::types::swagger::swagger_spec};

const NETLIFY_LOGS_URL_KEY: &str = "NETLIFY_LOGS_URL";
const NETLIFY_URL: &str = "https://app.netlify.com/sites/bencher/deploys/";

#[derive(Debug)]
pub struct NetlifyTest {
    pub dev: bool,
}

impl TryFrom<CliNetlifyTest> for NetlifyTest {
    type Error = anyhow::Error;

    fn try_from(swagger: CliNetlifyTest) -> Result<Self, Self::Error> {
        let CliNetlifyTest { dev } = swagger;
        Ok(Self { dev })
    }
}

impl NetlifyTest {
    pub async fn exec(&self) -> anyhow::Result<()> {
        let swagger_spec = swagger_spec()?;
        let Some(version) = swagger_spec.version() else {
            return Err(anyhow::anyhow!("No version found in swagger.json"));
        };

        let deploy_id = netlify_deploy_id("netlify.json")?;
        let console_url = if self.dev {
            format!("https://{deploy_id}--bencher.netlify.app")
        } else {
            PROD_BENCHER_URL_STR.to_owned()
        };
        test_ui_version(&console_url, version).await?;

        // TODO replace this with some actual e2e tests
        let project_slug = if self.dev { "the-computer" } else { "bencher" };
        let find_str = if self.dev {
            "<title>The Computer | Bencher - Continuous Benchmarking</title>"
        } else {
            "<title>Bencher | Bencher - Continuous Benchmarking</title>"
        };
        for i in 0..5 {
            if let Err(e) = test_ui_project(&console_url, project_slug, find_str).await {
                println!("Netlify deploy not ready yet: {e}");
                std::thread::sleep(std::time::Duration::from_secs(i));
            } else {
                break;
            }
        }

        Ok(())
    }
}

fn netlify_deploy_id(path: &str) -> anyhow::Result<String> {
    let netlify_path = Utf8PathBuf::from(path);
    let netlify_file = File::open(netlify_path)?;
    let netlify_output_json: serde_json::Value = serde_json::from_reader(netlify_file)?;

    let Some(logs_url) = netlify_output_json.get(NETLIFY_LOGS_URL_KEY) else {
        return Err(anyhow::anyhow!(
            "Netlify output did not contain {NETLIFY_LOGS_URL_KEY} key: {netlify_output_json}"
        ));
    };
    let Some(logs_url_str) = logs_url.as_str() else {
        return Err(anyhow::anyhow!("Netlify output {logs_url} is not a string"));
    };
    let Some((_, id)) = logs_url_str.split_once(NETLIFY_URL) else {
        return Err(anyhow::anyhow!(
            "Netlify logs URL {logs_url_str} does not match {NETLIFY_URL}"
        ));
    };

    println!("Netlify Deploy ID: {id}");
    Ok(id.to_owned())
}

async fn test_ui_version(console_url: &str, version: &str) -> anyhow::Result<()> {
    println!("Testing UI deploy is version {version} at {console_url}");
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

    Ok(())
}

async fn test_ui_project(
    console_url: &str,
    project_slug: &str,
    find_str: &str,
) -> anyhow::Result<()> {
    let url = format!("{console_url}/perf/{project_slug}");
    println!("Testing UI project {project_slug} at {url}");
    let html = reqwest::get(url).await?.text().await?;

    if !html.contains(find_str) {
        return Err(anyhow::anyhow!(
            "Console project ({project_slug}) page does not contain `{find_str}`: {html}"
        ));
    }

    Ok(())
}
