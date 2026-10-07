use std::{
    fs::{self, File},
    io::{BufRead as _, BufReader},
    net::TcpListener,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use futures_concurrency::future::Race as _;

use crate::parser::TaskE2e;

mod seed;

/// How long a server gets to answer its first request.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(60);
const POLL_INTERVAL: Duration = Duration::from_millis(100);
/// Lines of each server's log shown when the suite fails.
const LOG_TAIL: usize = 200;

/// The console end-to-end suite, run against an API on a fresh database.
#[derive(Debug)]
pub struct E2e {
    skip_console_build: bool,
    playwright: Vec<String>,
}

impl From<TaskE2e> for E2e {
    fn from(task: TaskE2e) -> Self {
        let TaskE2e {
            skip_console_build,
            playwright,
        } = task;
        Self {
            skip_console_build,
            playwright,
        }
    }
}

impl E2e {
    pub async fn exec(&self) -> anyhow::Result<()> {
        let root = workspace_root();
        let console_dir = root.join("services/console");

        let api_bin = build_api(&root)?;
        if !self.skip_console_build {
            build_console(&console_dir)?;
        }

        let temp_dir = tempfile::tempdir()?;
        let dir = Utf8Path::from_path(temp_dir.path())
            .context("The temporary directory is not UTF-8")?
            .to_owned();
        let mut servers = Servers::new(&dir);
        // Listening before the servers start, a signal ends the run and `servers`
        // still drops, instead of the harness dying and leaving them running.
        let stopped = stop_signal()?;
        let suite = async {
            let result = self.run(&mut servers, &dir, &api_bin, &console_dir).await;
            if result.is_err() {
                servers.print_logs();
            }
            result
        };
        (suite, stopped).race().await
    }

    async fn run(
        &self,
        servers: &mut Servers,
        dir: &Utf8Path,
        api_bin: &Utf8Path,
        console_dir: &Utf8Path,
    ) -> anyhow::Result<()> {
        let api_url = format!("http://127.0.0.1:{}", free_port()?);
        let console_url = format!("http://127.0.0.1:{}", free_port()?);

        servers.api = Some(spawn_api(
            api_bin,
            dir,
            &api_url,
            &console_url,
            &servers.api_log,
        )?);
        wait_until_up(&format!("{api_url}/v0/server/version")).await?;

        let seed = seed::Seed::new(&api_url, &console_url).await?;
        let seed_path = dir.join("seed.json");
        fs::write(&seed_path, serde_json::to_string_pretty(&seed)?)?;

        servers.console = Some(spawn_console(
            console_dir,
            &api_url,
            &console_url,
            &servers.console_log,
        )?);
        wait_until_up(&console_url).await?;

        println!("Running Playwright against {console_url}");
        // Not through `npx`, whose child would outlive it when a stop kills it.
        let playwright = console_dir.join("node_modules/.bin/playwright");
        let status = tokio::process::Command::new(playwright)
            .arg("test")
            .args(&self.playwright)
            .current_dir(console_dir)
            .env("BENCHER_E2E_SEED", &seed_path)
            .kill_on_drop(true)
            .status()
            .await
            .context("Failed to run Playwright")?;
        anyhow::ensure!(status.success(), "Playwright failed: {status}");
        Ok(())
    }
}

/// The API and console processes, killed when dropped.
struct Servers {
    api: Option<Child>,
    api_log: Utf8PathBuf,
    console: Option<Child>,
    console_log: Utf8PathBuf,
}

impl Servers {
    fn new(dir: &Utf8Path) -> Self {
        Self {
            api: None,
            api_log: dir.join("api.log"),
            console: None,
            console_log: dir.join("console.log"),
        }
    }

    fn print_logs(&self) {
        for (name, path) in [("API", &self.api_log), ("Console", &self.console_log)] {
            let Ok(log) = fs::read_to_string(path) else {
                continue;
            };
            let lines = log.lines().collect::<Vec<_>>();
            let start = lines.len().saturating_sub(LOG_TAIL);
            println!("==== {name} log ({path}) ====");
            for line in lines.iter().skip(start) {
                println!("{line}");
            }
        }
    }
}

impl Drop for Servers {
    fn drop(&mut self) {
        for child in [self.console.as_mut(), self.api.as_mut()]
            .into_iter()
            .flatten()
        {
            // A server that already exited has nothing left to kill or reap.
            if child.kill().is_ok() {
                drop(child.wait());
            }
        }
    }
}

fn workspace_root() -> Utf8PathBuf {
    Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Build the API and return the path to its binary, as Cargo reports it.
fn build_api(root: &Utf8Path) -> anyhow::Result<Utf8PathBuf> {
    println!("Building the API");
    let mut child = Command::new("cargo")
        .args([
            "build",
            "--package",
            "bencher_api",
            "--bin",
            "api",
            "--message-format=json-render-diagnostics",
        ])
        .current_dir(root)
        .stdout(Stdio::piped())
        .spawn()
        .context("Failed to run cargo build")?;
    let stdout = child.stdout.take().context("No cargo build output")?;
    let mut executable = None;
    for line in BufReader::new(stdout).lines() {
        let message: serde_json::Value = serde_json::from_str(&line?)?;
        if let Some(path) = message
            .get("executable")
            .and_then(serde_json::Value::as_str)
        {
            executable = Some(Utf8PathBuf::from(path));
        }
    }
    let status = child.wait()?;
    anyhow::ensure!(status.success(), "Failed to build the API: {status}");
    executable.context("Cargo did not report the API binary")
}

/// Build the console with the Node adapter, leaving `astro.config.mjs` as it was.
fn build_console(console_dir: &Utf8Path) -> anyhow::Result<()> {
    println!("Building the console");
    npm(console_dir, &["run", "wasm"])?;
    let config_path = console_dir.join("astro.config.mjs");
    let config = fs::read_to_string(&config_path)?;
    let built = npm(console_dir, &["run", "adapter", "node"])
        .and_then(|()| npm(console_dir, &["run", "build"]));
    fs::write(&config_path, config)?;
    built
}

fn npm(dir: &Utf8Path, args: &[&str]) -> anyhow::Result<()> {
    let status = Command::new("npm")
        .args(args)
        .current_dir(dir)
        .status()
        .with_context(|| format!("Failed to run npm {}", args.join(" ")))?;
    anyhow::ensure!(status.success(), "npm {} failed: {status}", args.join(" "));
    Ok(())
}

fn free_port() -> anyhow::Result<u16> {
    Ok(TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}

fn spawn_api(
    api_bin: &Utf8Path,
    dir: &Utf8Path,
    api_url: &str,
    console_url: &str,
    log: &Utf8Path,
) -> anyhow::Result<Child> {
    let bind_address = api_url.trim_start_matches("http://");
    // The issuer and secret are the debug defaults the test tokens are signed with.
    let config = serde_json::json!({
        "console": { "url": console_url },
        "security": {
            "issuer": "http://localhost:3000/",
            "secret_key": "DO_NOT_USE_THIS_IN_PRODUCTION",
        },
        "server": {
            "bind_address": bind_address,
            "request_body_max_bytes": 1 << 22,
        },
        "logging": {
            "name": "Bencher API",
            "log": { "stderr_terminal": { "level": "info" } },
        },
        "database": { "file": dir.join("bencher.db") },
    });
    let log = File::create(log)?;
    Command::new(api_bin)
        .current_dir(dir)
        .env("BENCHER_CONFIG", config.to_string())
        .env("BENCHER_CONFIG_PATH", dir.join("bencher.json"))
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()
        .context("Failed to start the API")
}

fn spawn_console(
    console_dir: &Utf8Path,
    api_url: &str,
    console_url: &str,
    log: &Utf8Path,
) -> anyhow::Result<Child> {
    let port = console_url.rsplit(':').next().context("No console port")?;
    let log = File::create(log)?;
    Command::new("node")
        .arg("dist/server/entry.mjs")
        .current_dir(console_dir)
        .env("HOST", "127.0.0.1")
        .env("PORT", port)
        .env("BENCHER_API_URL", api_url)
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()
        .context("Failed to start the console")
}

async fn wait_until_up(url: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    while Instant::now() < deadline {
        if client
            .get(url)
            .send()
            .await
            .is_ok_and(|resp| resp.status().is_success())
        {
            return Ok(());
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    anyhow::bail!(
        "{url} did not answer within {} seconds",
        STARTUP_TIMEOUT.as_secs()
    )
}

/// Listen for SIGINT and SIGTERM from now on, failing with the first to arrive.
#[cfg(unix)]
fn stop_signal() -> anyhow::Result<impl Future<Output = anyhow::Result<()>>> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;
    Ok(async move {
        let name = (
            async {
                interrupt.recv().await;
                "SIGINT"
            },
            async {
                terminate.recv().await;
                "SIGTERM"
            },
        )
            .race()
            .await;
        Err(anyhow::anyhow!("Stopped by {name}"))
    })
}

/// Listen for Ctrl-C from now on, failing when it arrives.
#[cfg(windows)]
fn stop_signal() -> anyhow::Result<impl Future<Output = anyhow::Result<()>>> {
    use tokio::signal::windows::ctrl_c;

    let mut ctrl_c = ctrl_c()?;
    Ok(async move {
        ctrl_c.recv().await;
        Err(anyhow::anyhow!("Stopped by Ctrl-C"))
    })
}
