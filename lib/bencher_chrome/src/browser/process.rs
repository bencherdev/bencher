use std::{
    borrow::BorrowMut,
    ffi::OsStr,
    io::{prelude::*, BufRead, BufReader},
    net,
    process::{Child, Command, Stdio},
    time::Duration,
};

#[cfg(test)]
use std::cell::RefCell;

use anyhow::{anyhow, Result};
use log::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use regex::Regex;
use url::Url;

use crate::browser::default_executable;
use crate::wait;

use std::collections::HashMap;

use crate::ChromeError;

#[cfg(test)]
struct ForTesting;
#[cfg(test)]
impl ForTesting {
    thread_local! {
        static USER_DATA_DIR: RefCell<Option<String>> = RefCell::new(None);
    }
}

pub struct Process {
    child_process: TemporaryProcess,
    pub debug_ws_url: Url,
}

struct TemporaryProcess(Child, Option<tempfile::TempDir>);

impl Drop for TemporaryProcess {
    fn drop(&mut self) {
        info!("Killing Chrome. PID: {}", self.0.id());
        self.0.kill().and_then(|_| self.0.wait()).ok();
        if let Some(dir) = self.1.take() {
            if let Err(e) = dir.close() {
                warn!("Failed to close temporary directory: {}", e);
            }
        };
    }
}

/// Represents the way in which Chrome is run. By default it will search for a Chrome
/// binary on the system, use an available port for debugging, and start in headless mode.
#[derive(Debug, Builder)]
pub struct LaunchOptions<'a> {
    /// Determines whether to run headless version of the browser. Defaults to true.
    #[builder(default = "true")]
    pub headless: bool,

    /// Determines whether to run the browser with a sandbox.
    #[builder(default = "true")]
    pub sandbox: bool,

    /// Launch the browser with a specific window width and height.
    #[builder(default = "None")]
    pub window_size: Option<(u32, u32)>,

    /// Launch the browser with a specific debugging port.
    #[builder(default = "None")]
    pub port: Option<u16>,
    /// Determines whether SSL certificates should be verified.
    /// This is unsafe and can lead to MiTM attacks. Make sure you understand the risks
    /// See <https://www.owasp.org/index.php/Man-in-the-middle_attack>
    #[builder(default = "true")]
    pub ignore_certificate_errors: bool,

    /// Path for Chrome or Chromium.
    ///
    /// If unspecified, the create will try to automatically detect a suitable binary.
    #[builder(default = "None")]
    pub path: Option<std::path::PathBuf>,

    /// User Data (Profile) to use.
    ///
    /// If unspecified, a new temp directory is created and used on every launch.
    #[builder(default = "None")]
    pub user_data_dir: Option<std::path::PathBuf>,

    /// A list of Chrome extensions to load.
    ///
    /// An extension should be a path to a folder containing the extension code.
    /// CRX files cannot be used directly and must be first extracted.
    ///
    /// Note that Chrome does not support loading extensions in headless-mode.
    /// See <https://bugs.chromium.org/p/chromium/issues/detail?id=706008#c5>
    #[builder(default)]
    pub extensions: Vec<&'a OsStr>,

    /// Additional arguments to pass to the browser instance. The list of Chromium
    /// flags can be found: <http://peter.sh/experiments/chromium-command-line-switches/>.
    #[builder(default)]
    pub args: Vec<&'a OsStr>,

    /// Disable default arguments
    #[builder(default)]
    pub disable_default_args: bool,

    /// How long to keep the WebSocket to the browser for after not receiving any events from it
    /// Defaults to u64::MAX
    #[builder(default = "Duration::from_secs(u64::MAX)")]
    pub idle_browser_timeout: Duration,

    /// Environment variables to set for the Chromium process.
    /// Passes value through to std::process::Command::envs.
    #[builder(default = "None")]
    pub process_envs: Option<HashMap<String, String>>,

    /// Setup the proxy server for headless chrome instance
    #[builder(default = "None")]
    pub proxy_server: Option<&'a str>,

    /// Channel buffer size
    pub channel_buffer: usize,

    /// Timeout
    #[builder(default = "Duration::from_millis(5_000)")]
    pub timeout: Duration,

    /// Sleep
    #[builder(default = "Duration::from_millis(1)")]
    pub sleep: Duration,
}

impl<'a> Default for LaunchOptions<'a> {
    fn default() -> Self {
        LaunchOptions {
            headless: true,
            sandbox: true,
            idle_browser_timeout: Duration::from_secs(u64::MAX),
            window_size: None,
            path: None,
            user_data_dir: None,
            port: None,
            ignore_certificate_errors: true,
            extensions: Vec::new(),
            process_envs: None,
            args: Vec::new(),
            disable_default_args: false,
            proxy_server: None,
            channel_buffer: 1024,
            timeout: Duration::from_millis(5_000),
            sleep: Duration::from_millis(1),
        }
    }
}

impl<'a> LaunchOptions<'a> {
    pub fn default_builder() -> LaunchOptionsBuilder<'a> {
        LaunchOptionsBuilder::default()
    }
}

/// These are passed to the Chrome binary by default.
/// Via <https://github.com/GoogleChrome/puppeteer/blob/master/lib/Launcher.js#L38>
pub static DEFAULT_ARGS: [&str; 23] = [
    "--disable-background-networking",
    "--enable-features=NetworkService,NetworkServiceInProcess",
    "--disable-background-timer-throttling",
    "--disable-backgrounding-occluded-windows",
    "--disable-breakpad",
    "--disable-client-side-phishing-detection",
    "--disable-component-extensions-with-background-pages",
    "--disable-default-apps",
    "--disable-dev-shm-usage",
    "--disable-extensions",
    // BlinkGenPropertyTrees disabled due to crbug.com/937609
    "--disable-features=TranslateUI,BlinkGenPropertyTrees",
    "--disable-hang-monitor",
    "--disable-ipc-flooding-protection",
    "--disable-popup-blocking",
    "--disable-prompt-on-repost",
    "--disable-renderer-backgrounding",
    "--disable-sync",
    "--force-color-profile=srgb",
    "--metrics-recording-only",
    "--no-first-run",
    "--enable-automation",
    "--password-store=basic",
    "--use-mock-keychain",
];

impl Process {
    pub async fn new(mut launch_options: LaunchOptions<'static>) -> Result<Self, ChromeError> {
        if launch_options.path.is_none() {
            launch_options.path = Some(default_executable()?);
        }

        let mut process = Self::start_process(&launch_options)?;

        info!("Started Chrome. PID: {}", process.0.id());

        let url;
        let mut attempts = 0;
        loop {
            if attempts > 10 {
                return Err(ChromeError::NoAvailablePorts);
            }

            match Self::ws_url_from_output(process.0.borrow_mut()) {
                Ok(debug_ws_url) => {
                    url = debug_ws_url;
                    debug!("Found debugging WS URL: {:?}", url);
                    break;
                },
                Err(error) => {
                    trace!("Problem getting WebSocket URL from Chrome: {}", error);
                    if launch_options.port.is_none() {
                        process = Self::start_process(&launch_options)?;
                    } else {
                        return Err(error);
                    }
                },
            }

            trace!(
                "Trying again to find available debugging port. Attempts: {}",
                attempts
            );
            attempts += 1;
        }

        let mut child = process.0.borrow_mut();
        child.stderr = None;

        Ok(Self {
            child_process: process,
            debug_ws_url: url,
        })
    }

    fn start_process(launch_options: &LaunchOptions) -> Result<TemporaryProcess> {
        let debug_port = if let Some(port) = launch_options.port {
            port
        } else {
            get_available_port().ok_or(ChromeLaunchError::NoAvailablePorts {})?
        };
        let port_option = format!("--remote-debugging-port={debug_port}");

        let window_size_option = if let Some((width, height)) = launch_options.window_size {
            format!("--window-size={width},{height}")
        } else {
            String::new()
        };

        let mut temp_user_data_dir = None;

        // User data directory
        let user_data_dir = if let Some(dir) = &launch_options.user_data_dir {
            dir.clone()
        } else {
            // picking random data dir so that each a new browser instance is launched
            // (see man google-chrome)
            let dir = ::tempfile::Builder::new()
                .prefix("rust-headless-chrome-profile")
                .tempdir()?;

            let buf = dir.path().to_path_buf();
            temp_user_data_dir = Some(dir);
            buf
        };
        let data_dir_option = format!("--user-data-dir={}", &user_data_dir.to_str().unwrap());

        #[cfg(test)]
        ForTesting::USER_DATA_DIR.with(|dir| {
            *dir.borrow_mut() = user_data_dir.to_str().map(std::borrow::ToOwned::to_owned);
        });

        trace!("Chrome will have profile: {}", data_dir_option);

        let mut args = vec![
            port_option.as_str(),
            "--disable-gpu",
            "--enable-logging",
            "--verbose",
            "--log-level=0",
            "--no-first-run",
            "--disable-audio-output",
            data_dir_option.as_str(),
        ];

        if !launch_options.disable_default_args {
            args.extend(DEFAULT_ARGS);
        }

        if !launch_options.args.is_empty() {
            let extra_args: Vec<&str> = launch_options
                .args
                .iter()
                .map(|a| a.to_str().unwrap())
                .collect();
            args.extend(extra_args);
        }

        if !window_size_option.is_empty() {
            args.extend([window_size_option.as_str()]);
        }

        if launch_options.headless {
            args.extend(["--headless"]);
        }

        if launch_options.ignore_certificate_errors {
            args.extend(["--ignore-certificate-errors"]);
        }

        let proxy_server_option = if let Some(proxy_server) = launch_options.proxy_server {
            format!("--proxy-server={proxy_server}")
        } else {
            String::new()
        };

        if !proxy_server_option.is_empty() {
            args.extend([proxy_server_option.as_str()]);
        }

        if !launch_options.sandbox {
            args.extend(["--no-sandbox", "--disable-setuid-sandbox"]);
        }

        let extension_args: Vec<String> = launch_options
            .extensions
            .iter()
            .map(|e| format!("--load-extension={}", e.to_str().unwrap()))
            .collect();

        args.extend(extension_args.iter().map(String::as_str));

        let path = launch_options
            .path
            .as_ref()
            .ok_or_else(|| anyhow!("Chrome path required"))?;

        info!("Launching Chrome binary at {:?}", &path);
        trace!("with CLI arguments: {:?}", args);

        let mut command = Command::new(path);

        if let Some(process_envs) = launch_options.process_envs.clone() {
            command.envs(process_envs);
        }

        let process = TemporaryProcess(
            command.args(&args).stderr(Stdio::piped()).spawn()?,
            temp_user_data_dir,
        );
        Ok(process)
    }

    fn ws_url_from_reader<R>(reader: BufReader<R>) -> Result<Option<String>>
    where
        R: Read,
    {
        let port_taken_re = Regex::new(r"ERROR.*bind\(\)")?;

        let re = Regex::new(r"listening on (.*/devtools/browser/.*)$")?;

        let extract = |text: &str| -> Option<String> {
            let caps = re.captures(text);
            let cap = &caps?[1];
            Some(cap.into())
        };

        for line in reader.lines() {
            let chrome_output = line?;
            trace!("Chrome output: {}", chrome_output);

            if port_taken_re.is_match(&chrome_output) {
                return Err(ChromeLaunchError::DebugPortInUse {}.into());
            }

            if let Some(answer) = extract(&chrome_output) {
                return Ok(Some(answer));
            }
        }

        Ok(None)
    }

    fn ws_url_from_output(child_process: &mut Child) -> Result<Url> {
        let chrome_output_result = wait::Wait::with_timeout(Duration::from_secs(30)).until(|| {
            let my_stderr = BufReader::new(child_process.stderr.as_mut()?);
            match Self::ws_url_from_reader(my_stderr) {
                Ok(output_option) => output_option.map(Ok),
                Err(err) => Some(Err(err)),
            }
        });

        if let Ok(output_result) = chrome_output_result {
            Ok(Url::parse(&output_result?)?)
        } else {
            Err(ChromeLaunchError::PortOpenTimeout {}.into())
        }
    }

    pub fn get_id(&self) -> u32 {
        self.child_process.0.id()
    }
}

fn get_available_port() -> Option<u16> {
    let mut ports: Vec<u16> = (8000..9000).collect();
    ports.shuffle(&mut thread_rng());
    ports.iter().find(|port| port_is_available(**port)).copied()
}

fn port_is_available(port: u16) -> bool {
    net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

#[cfg(test)]
mod tests {
    use std::sync::Once;
    use std::thread;

    use crate::browser::default_executable;

    use super::*;

    static INIT: Once = Once::new();

    fn setup() {
        INIT.call_once(|| {
            env_logger::try_init().unwrap_or(());
        });
    }

    #[tokio::test]
    async fn can_launch_chrome_and_get_ws_url() {
        setup();
        let chrome = super::Process::new(
            LaunchOptions::default_builder()
                .path(Some(default_executable().await.unwrap()))
                .build()
                .unwrap(),
        )
        .await
        .unwrap();
        info!("{:?}", chrome.debug_ws_url);
    }

    #[test]
    fn handle_errors_in_chrome_output() {
        setup();
        let lines = "[0228/194641.093619:ERROR:socket_posix.cc(144)] bind() returned an error, errno=0: Cannot assign requested address (99)";
        let reader = BufReader::new(lines.as_bytes());
        let ws_url_result = Process::ws_url_from_reader(reader);
        assert!(ws_url_result.is_err());
    }

    #[test]
    fn handle_errors_in_chrome_output_gvisor_netlink() {
        // see https://github.com/atroche/rust-headless-chrome/issues/261
        setup();
        let lines = "[0703/145506.975691:ERROR:address_tracker_linux.cc(214)] Could not bind NETLINK socket: Permission denied (13)";

        let reader = BufReader::new(lines.as_bytes());
        let ws_url_result = Process::ws_url_from_reader(reader);
        assert!(ws_url_result.is_ok());
    }

    #[cfg(target_os = "linux")]
    fn current_child_pids() -> Vec<i32> {
        use std::fs::File;
        use std::io::prelude::*;
        let current_pid = std::process::id();
        let mut current_process_children_file =
            File::open(format!("/proc/{current_pid}/task/{current_pid}/children")).unwrap();
        let mut child_pids = String::new();
        current_process_children_file
            .read_to_string(&mut child_pids)
            .unwrap();
        child_pids
            .split_whitespace()
            .map(|pid_str| pid_str.parse::<i32>().unwrap())
            .collect()
    }

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn kills_process_on_drop() {
        setup();
        {
            let _chrome = &mut super::Process::new(
                LaunchOptions::default_builder()
                    .path(Some(default_executable().await.unwrap()))
                    .build()
                    .unwrap(),
            )
            .await
            .unwrap();
        }

        let child_pids = current_child_pids();
        assert!(child_pids.is_empty());
    }

    #[tokio::test]
    async fn launch_multiple_non_headless_instances() {
        setup();
        let mut handles = Vec::new();

        for _ in 0..10 {
            let handle = thread::spawn(|| {
                // these sleeps are to make it more likely the chrome startups will overlap
                std::thread::sleep(std::time::Duration::from_millis(10));
                let chrome = super::Process::new(
                    LaunchOptions::default_builder()
                        .path(Some(default_executable().await.unwrap()))
                        .build()
                        .unwrap(),
                )
                .await
                .unwrap();
                std::thread::sleep(std::time::Duration::from_millis(100));
                chrome.debug_ws_url
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[tokio::test]
    async fn no_instance_sharing() {
        setup();

        let mut handles = Vec::new();

        for _ in 0..10 {
            let chrome = super::Process::new(
                LaunchOptions::default_builder()
                    .path(Some(default_executable().await.unwrap()))
                    .headless(true)
                    .build()
                    .unwrap(),
            )
            .await
            .unwrap();
            handles.push(chrome);
        }
    }

    #[tokio::test]
    async fn test_temporary_user_data_dir_is_removed_automatically() {
        setup();

        let options = LaunchOptions::default_builder().build().unwrap();

        // Ensure we did not pass an explicit user_data_dir
        let temp_dir = options.user_data_dir.clone();
        assert_eq!(None, temp_dir);

        let user_data_dir = {
            let _chrome = &mut super::Process::new(options).await.unwrap();

            ForTesting::USER_DATA_DIR.with(|dir| dir.borrow_mut().take())
        };

        match user_data_dir {
            Some(temp_path) => {
                let user_data_dir_exists = std::path::Path::new(&temp_path).is_dir();
                assert!(!user_data_dir_exists);
            },
            None => panic!("No user data dir was created"),
        }
    }
}
