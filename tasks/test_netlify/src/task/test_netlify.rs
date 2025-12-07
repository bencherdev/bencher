use std::{fs::File, thread, time::Duration};

use bencher_json::PROD_BENCHER_URL_STR;
use camino::Utf8PathBuf;
use serde::Serialize;

use crate::{API_VERSION, parser::TaskTestNetlify};

const DEPLOY_ID_KEY: &str = "deploy_id";
const NTFY_URL: &str = "https://ntfy.sh";
const NTFY_TOPIC: &str = "bencherdev";

#[derive(Debug)]
pub struct TestNetlify {
    pub dev: bool,
    pub ref_name: String,
}

impl TestNetlify {
    pub fn dev(test_netlify: TaskTestNetlify) -> Self {
        let TaskTestNetlify { ref_name } = test_netlify;
        Self {
            dev: true,
            ref_name,
        }
    }

    pub fn prod(test_netlify: TaskTestNetlify) -> Self {
        let TaskTestNetlify { ref_name } = test_netlify;
        Self {
            dev: false,
            ref_name,
        }
    }

    pub async fn exec(&self) -> anyhow::Result<()> {
        let deploy_id = netlify_deploy_id("netlify.json")?;
        let console_url = if self.dev {
            format!("https://{deploy_id}--bencher.netlify.app")
        } else {
            PROD_BENCHER_URL_STR.to_owned()
        };

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
                thread::sleep(Duration::from_secs(i));
            } else {
                break;
            }
        }
        test_ui_version(&console_url).await?;

        let notify = Notify::new(&self.ref_name, &console_url);
        notify.send().await?;

        Ok(())
    }
}

fn netlify_deploy_id(path: &str) -> anyhow::Result<String> {
    let netlify_path = Utf8PathBuf::from(path);
    let netlify_file = File::open(netlify_path)?;
    let netlify_output_json: serde_json::Value = serde_json::from_reader(netlify_file)?;

    let Some(deploy_id) = netlify_output_json.get(DEPLOY_ID_KEY) else {
        return Err(anyhow::anyhow!(
            "Netlify output did not contain {DEPLOY_ID_KEY} key: {netlify_output_json}"
        ));
    };
    let Some(deploy_id_str) = deploy_id.as_str() else {
        return Err(anyhow::anyhow!(
            "Netlify Deploy ID {deploy_id} is not a string"
        ));
    };

    println!("Netlify Deploy ID: {deploy_id_str}");
    Ok(deploy_id_str.to_owned())
}

async fn test_ui_project(
    console_url: &str,
    project_slug: &str,
    find_str: &str,
) -> anyhow::Result<()> {
    let url = format!("{console_url}/perf/{project_slug}");
    println!("Testing UI project {project_slug} at {url}");

    fetch_and_check(&url, find_str).await
}

async fn test_ui_version(console_url: &str) -> anyhow::Result<()> {
    let url = format!("{console_url}/download");
    println!("Testing UI deploy is version {API_VERSION} at {url}");

    let heading = format!("Bencher Download (v{API_VERSION})");

    fetch_and_check(&url, &heading).await
}

async fn fetch_and_check(url: &str, find_str: &str) -> anyhow::Result<()> {
    let html = reqwest::get(url).await?.text().await?;
    if !html.contains(find_str) {
        return Err(anyhow::anyhow!(
            "Failed to find `{find_str}` in HTML from {url}"
        ));
    }

    Ok(())
}

#[derive(Debug, Serialize)]
pub struct Notify {
    topic: String,
    message: String,
    click: Option<String>,
}

impl Notify {
    pub fn new(ref_name: &str, console_url: &str) -> Self {
        Self {
            topic: NTFY_TOPIC.to_owned(),
            message: format!("Deployed {ref_name}"),
            click: Some(console_url.to_owned()),
        }
    }

    pub async fn send(&self) -> anyhow::Result<()> {
        let notify_json = serde_json::to_string(self)?;
        let client = reqwest::Client::new();
        let _resp = client.post(NTFY_URL).body(notify_json).send().await?;
        Ok(())
    }
}
