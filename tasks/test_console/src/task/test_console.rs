use std::{thread, time::Duration};

use bencher_json::{
    BmfVersion, DEV_BENCHER_API_URL_STR, DEVEL_BENCHER_URL_STR, JsonProject,
    PROD_BENCHER_API_URL_STR, PROD_BENCHER_URL_STR,
};
use serde::Serialize;

use crate::{API_VERSION, parser::TaskTestConsole};

const NTFY_URL: &str = "https://ntfy.sh";
const NTFY_TOPIC: &str = "bencherdev";
const USER_AGENT: &str = "bencher-test-console";
// Identifies the deploy smoke test to the console's bot challenge exemption
const AGENT_HEADER: &str = "x-bencher-agent";

#[derive(Debug)]
pub struct TestConsole {
    pub dev: bool,
    pub ref_name: String,
    pub agent_key: Option<String>,
}

impl TestConsole {
    pub fn dev(test_console: TaskTestConsole) -> Self {
        let TaskTestConsole {
            ref_name,
            agent_key,
        } = test_console;
        Self {
            dev: true,
            ref_name,
            agent_key,
        }
    }

    pub fn prod(test_console: TaskTestConsole) -> Self {
        let TaskTestConsole {
            ref_name,
            agent_key,
        } = test_console;
        Self {
            dev: false,
            ref_name,
            agent_key,
        }
    }

    pub async fn exec(&self) -> anyhow::Result<()> {
        let (console_url, api_url) = if self.dev {
            (DEVEL_BENCHER_URL_STR, DEV_BENCHER_API_URL_STR)
        } else {
            (PROD_BENCHER_URL_STR, PROD_BENCHER_API_URL_STR)
        };

        // TODO replace this with some actual e2e tests
        let (project_slug, project_name) = if self.dev {
            ("the-computer", "The Computer")
        } else {
            ("bencher", "Bencher")
        };
        let mut result = Ok(());
        for i in 0..5 {
            match test_ui_project(
                console_url,
                api_url,
                project_slug,
                project_name,
                self.agent_key.as_deref(),
            )
            .await
            {
                Ok(()) => {
                    result = Ok(());
                    break;
                },
                Err(e) => {
                    println!("Console deploy not ready yet: {e}");
                    result = Err(e);
                    thread::sleep(Duration::from_secs(i));
                },
            }
        }
        result?;
        test_ui_version(console_url, self.agent_key.as_deref()).await?;

        let notify = Notify::new(&self.ref_name, console_url);
        notify.send().await?;

        Ok(())
    }
}

async fn test_ui_project(
    console_url: &str,
    api_url: &str,
    project_slug: &str,
    project_name: &str,
    agent_key: Option<&str>,
) -> anyhow::Result<()> {
    let project: JsonProject = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()?
        .get(format!("{api_url}/v0/projects/{project_slug}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let (url, find_str) = if project.bmf_version == BmfVersion::V1 {
        (
            format!("{console_url}/next/perf/{project_slug}"),
            format!("<title>{project_name} | Bencher</title>"),
        )
    } else {
        (
            format!("{console_url}/perf/{project_slug}"),
            format!("<title>{project_name} | Bencher - Continuous Benchmarking</title>"),
        )
    };
    println!(
        "Testing UI project {project_slug} (BMF version {}) at {url}",
        project.bmf_version
    );

    fetch_and_check(&url, agent_key, &find_str).await
}

async fn test_ui_version(console_url: &str, agent_key: Option<&str>) -> anyhow::Result<()> {
    let url = format!("{console_url}/download");
    println!("Testing UI deploy is version {API_VERSION} at {url}");

    let version = format!("Latest Version: <code>v{API_VERSION}</code>");

    fetch_and_check(&url, agent_key, &version).await
}

async fn fetch_and_check(url: &str, agent_key: Option<&str>, find_str: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::builder().user_agent(USER_AGENT).build()?;
    let request = client.get(url);
    let request = if let Some(agent_key) = agent_key {
        request.header(AGENT_HEADER, agent_key)
    } else {
        request
    };
    let response = request.send().await?;
    let final_url = response.url().clone();
    let html = response.text().await?;
    if !html.contains(find_str) {
        return Err(anyhow::anyhow!(
            "Failed to find `{find_str}` in HTML from {final_url}"
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
