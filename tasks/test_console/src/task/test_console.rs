use std::{thread, time::Duration};

use bencher_json::{DEVEL_BENCHER_URL_STR, PROD_BENCHER_URL_STR};
use serde::Serialize;

use crate::{API_VERSION, parser::TaskTestConsole};

const NTFY_URL: &str = "https://ntfy.sh";
const NTFY_TOPIC: &str = "bencherdev";

#[derive(Debug)]
pub struct TestConsole {
    pub dev: bool,
    pub ref_name: String,
    pub user_agent: Option<String>,
}

impl TestConsole {
    pub fn dev(test_console: TaskTestConsole) -> Self {
        let TaskTestConsole {
            ref_name,
            user_agent,
        } = test_console;
        Self {
            dev: true,
            ref_name,
            user_agent,
        }
    }

    pub fn prod(test_console: TaskTestConsole) -> Self {
        let TaskTestConsole {
            ref_name,
            user_agent,
        } = test_console;
        Self {
            dev: false,
            ref_name,
            user_agent,
        }
    }

    pub async fn exec(&self) -> anyhow::Result<()> {
        let console_url = if self.dev {
            DEVEL_BENCHER_URL_STR
        } else {
            PROD_BENCHER_URL_STR
        };

        // TODO replace this with some actual e2e tests
        let project_slug = if self.dev { "the-computer" } else { "bencher" };
        let find_str = if self.dev {
            "<title>The Computer | Bencher - Continuous Benchmarking</title>"
        } else {
            "<title>Bencher | Bencher - Continuous Benchmarking</title>"
        };
        let mut result = Ok(());
        for i in 0..5 {
            match test_ui_project(
                console_url,
                project_slug,
                self.user_agent.as_deref(),
                find_str,
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
        test_ui_version(console_url, self.user_agent.as_deref()).await?;

        let notify = Notify::new(&self.ref_name, console_url);
        notify.send().await?;

        Ok(())
    }
}

async fn test_ui_project(
    console_url: &str,
    project_slug: &str,
    user_agent: Option<&str>,
    find_str: &str,
) -> anyhow::Result<()> {
    let url = format!("{console_url}/perf/{project_slug}");
    println!("Testing UI project {project_slug} at {url}");

    fetch_and_check(&url, user_agent, find_str).await
}

async fn test_ui_version(console_url: &str, user_agent: Option<&str>) -> anyhow::Result<()> {
    let url = format!("{console_url}/download");
    println!("Testing UI deploy is version {API_VERSION} at {url}");

    let version = format!("Latest Version: <code>v{API_VERSION}</code>");

    fetch_and_check(&url, user_agent, &version).await
}

async fn fetch_and_check(
    url: &str,
    user_agent: Option<&str>,
    find_str: &str,
) -> anyhow::Result<()> {
    let client = if let Some(user_agent) = user_agent {
        reqwest::Client::builder().user_agent(user_agent).build()?
    } else {
        reqwest::Client::new()
    };
    let html = client.get(url).send().await?.text().await?;
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
