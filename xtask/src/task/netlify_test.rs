use std::{
    fs::File,
    io::{BufRead, BufReader},
};

use camino::Utf8PathBuf;

use crate::{parser::CliNetlifyTest, task::swagger::swagger_spec};

const CONSOLE_URL: &str = "https://bencher.dev";
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
        if !self.dev {
            test_ui_version(CONSOLE_URL, version).await?;
        }

        let deploy_id = netlify_deploy_id("netlify.txt")?;
        let console_url = format!("https://{deploy_id}--bencher.netlify.app");
        test_ui_version(&console_url, version).await?;

        Ok(())
    }
}

fn netlify_deploy_id(path: &str) -> anyhow::Result<String> {
    let netlify_path = Utf8PathBuf::from(path);
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

    deploy_id.ok_or(anyhow::anyhow!("No Netlify Deploy ID found"))
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
