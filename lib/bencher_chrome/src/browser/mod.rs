use std::sync::Arc;
use std::sync::Mutex;

use log::{debug, error, info, trace};

use process::Process;
pub use process::{LaunchOptions, LaunchOptionsBuilder, DEFAULT_ARGS};
pub use tab::Tab;
use tokio::sync::mpsc;
use transport::Transport;
use url::Url;
use which::which;

use crate::protocol::cdp::{types::Event, types::Method, Browser as B, Target, CSS, DOM};

use crate::browser::context::Context;
use crate::wait;
use Target::{CreateTarget, SetDiscoverTargets};
use B::GetVersion;
pub use B::GetVersionReturnObject;

pub mod context;
mod process;
pub mod tab;
pub mod transport;

// new
use tokio::time::Duration;

use crate::ChromeError;

/// A handle to an instance of Chrome / Chromium, which wraps a WebSocket connection to its debugging port.
///
///
/// Most of your actual "driving" (e.g. clicking, typing, navigating) will be via instances of [Tab](../tab/struct.Tab.html), which are accessible via methods such as `get_tabs`.
///
/// A Browser can either manage its own Chrome process or connect to a remote one.
///
/// `Browser::default().unwrap()` will return a headless instance of whatever browser can be found using
/// `default_executable`, which will search on your PATH for relevant binaries or use the path
/// specified in the `CHROME` env var.
///
/// You can use [LaunchOptions](../process/LaunchOptions.struct.html) to automatically
/// download a revision of Chromium that has a compatible API into your `$XDG_DATA_DIR`. Alternatively,
/// you can specify your own path to a binary, or make use of the `default_executable` function to use
///  your already-installed copy of Chrome.
///
/// While the Chrome DevTools Protocol (CDTP) does define some methods in a
/// ["Browser" domain](https://chromedevtools.github.io/devtools-protocol/tot/Browser)
/// (such as for resizing the window in non-headless mode), we currently don't implement those.
#[derive(Clone)]
pub struct Browser {
    inner: Arc<BrowserInner>,
}

pub struct BrowserInner {
    process: Option<Process>,
    transport: Arc<Transport>,
    tabs: Arc<Mutex<Vec<Arc<Tab>>>>,
    loop_shutdown_tx: mpsc::Sender<()>,
}

impl Browser {
    /// Launch a new Chrome browser.
    ///
    /// The browser will have its user data (aka "profile") directory stored in a temporary directory.
    /// The browser process will be killed when this struct is dropped.
    pub async fn new(launch_options: LaunchOptions<'static>) -> Result<Self, ChromeError> {
        let process = Process::new(launch_options).await?;
        let transport = Arc::new(
            Transport::new(
                process.debug_ws_url.clone(),
                Some(process.get_id()),
                launch_options.idle_browser_timeout,
                launch_options.channel_buffer,
            )
            .await?,
        );

        Self::create_browser(
            Some(process),
            transport,
            launch_options.idle_browser_timeout,
            launch_options.timeout,
            launch_options.sleep,
            launch_options.channel_buffer,
        )
        .await
    }

    /// Calls [`Browser::new`] with options to launch a headless browser using whatever Chrome / Chromium
    /// binary can be found on the system.
    pub async fn default() -> Result<Self, ChromeError> {
        let launch_options = LaunchOptions::default_builder()
            .path(Some(default_executable().await?))
            .build()?;
        Self::new(launch_options).await
    }

    /// Allows you to drive an externally-launched Chrome process instead of launch one via [`Browser::new`].
    /// If the browser is idle for `idle_browser_timeout`, the connection will be dropped.
    pub async fn connect(
        debug_ws_url: String,
        idle_browser_timeout: Duration,
        timeout: u64,
        sleep: u64,
        channel_buffer: usize,
    ) -> Result<Self, ChromeError> {
        let url = Url::parse(&debug_ws_url)?;

        let transport =
            Arc::new(Transport::new(url, None, idle_browser_timeout, channel_buffer).await?);
        trace!("created transport");

        Self::create_browser(
            None,
            transport,
            idle_browser_timeout,
            timeout,
            sleep,
            channel_buffer,
        )
        .await
    }

    async fn create_browser(
        process: Option<Process>,
        transport: Arc<Transport>,
        idle_browser_timeout: Duration,
        timeout: Duration,
        sleep: Duration,
        channel_buffer: usize,
    ) -> Result<Self, ChromeError> {
        let tabs = Arc::new(Mutex::new(vec![]));

        let (shutdown_tx, shutdown_rx) = mpsc::channel(64);

        let browser = Browser {
            inner: Arc::new(BrowserInner {
                process,
                tabs,
                transport,
                loop_shutdown_tx: shutdown_tx,
            }),
        };

        let incoming_events_rx = browser
            .inner
            .transport
            .listen_to_browser_events(channel_buffer);

        browser.handle_browser_level_events(
            incoming_events_rx,
            browser.get_process_id(),
            shutdown_rx,
            idle_browser_timeout,
        );
        trace!("created browser event listener");

        // so we get events like 'targetCreated' and 'targetDestroyed'
        trace!("Calling set discover");
        browser.call_method(SetDiscoverTargets { discover: true })?;

        let tab = browser.new_tab(timeout, sleep).await?;

        tab.call_method(DOM::Enable(None)).await?;
        tab.call_method(CSS::Enable(None)).await?;

        Ok(browser)
    }

    pub fn get_process_id(&self) -> Option<u32> {
        self.inner.process.as_ref().map(process::Process::get_id)
    }

    /// The tabs are behind an `Arc` and `Mutex` because they're accessible from multiple threads
    /// (including the one that handles incoming protocol events about new or changed tabs).
    pub fn get_tabs(&self) -> &Arc<Mutex<Vec<Arc<Tab>>>> {
        &self.inner.tabs
    }

    /// Create a new tab and return a handle to it.
    ///
    /// If you want to specify its starting options, see `new_tab_with_options`.
    pub async fn new_tab(
        &self,
        timeout: Duration,
        sleep: Duration,
    ) -> Result<Arc<Tab>, ChromeError> {
        let default_blank_tab = CreateTarget {
            url: "about:blank".to_string(),
            width: None,
            height: None,
            browser_context_id: None,
            enable_begin_frame_control: None,
            new_window: None,
            background: None,
        };
        self.new_tab_with_options(default_blank_tab, timeout, sleep)
            .await
    }

    /// Create a new tab with a starting url, height / width, context ID and 'frame control'
    pub fn new_tab_with_options(
        &self,
        create_target_params: CreateTarget,
        timeout: Duration,
        sleep: Duration,
    ) -> Result<Arc<Tab>, ChromeError> {
        let target_id = self.call_method(create_target_params).await?.target_id;

        wait::Wait::new(Duration::from_secs(timeout))
            .until(|| {
                let tabs = self.inner.tabs.lock().unwrap();
                tabs.iter().find_map(|tab| {
                    if *tab.get_target_id() == target_id {
                        Some(tab.clone())
                    } else {
                        None
                    }
                })
            })
            .await
            .map_err(Into::into)
    }

    /// Creates the equivalent of a new incognito window, AKA a browser context
    pub async fn new_context(&self) -> Result<context::Context, ChromeError> {
        debug!("Creating new browser context");
        let context_id = self
            .call_method(Target::CreateBrowserContext {
                dispose_on_detach: None,
                proxy_server: None,
                proxy_bypass_list: None,
                origins_with_universal_network_access: None,
            })
            .await?
            .browser_context_id;
        debug!("Created new browser context: {:?}", context_id);
        Ok(Context::new(self, context_id))
    }

    /// Get version information
    pub fn get_version(&self) -> Result<GetVersionReturnObject, ChromeError> {
        self.call_method(GetVersion(None))
    }

    async fn handle_browser_level_events(
        &self,
        events_rx: mpsc::Receiver<Event>,
        process_id: Option<u32>,
        shutdown_rx: mpsc::Receiver<()>,
        idle_browser_timeout: Duration,
    ) {
        let tabs = Arc::clone(&self.inner.tabs);
        let transport = Arc::clone(&self.inner.transport);

        tokio::task::spawn(move || {
            trace!("Starting browser's event handling loop");
            loop {
                match shutdown_rx.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        info!("Browser event loop received shutdown message");
                        break;
                    },
                    Err(TryRecvError::Empty) => {},
                }

                match events_rx.recv_timeout(idle_browser_timeout) {
                    Err(recv_timeout_error) => {
                        match recv_timeout_error {
                            RecvTimeoutError::Timeout => {
                                error!(
                                    "Got a timeout while listening for browser events (Chrome #{:?})",
                                    process_id
                                );
                            },
                            RecvTimeoutError::Disconnected => {
                                debug!(
                                    "Browser event sender disconnected while loop was waiting (Chrome #{:?})",
                                    process_id
                                );
                            },
                        }
                        break;
                    },
                    Ok(event) => {
                        match event {
                            Event::TargetCreated(ev) => {
                                let target_info = ev.params.target_info;
                                trace!("Creating target: {:?}", target_info);
                                if target_info.Type == "page" {
                                    match Tab::new(target_info, Arc::clone(&transport)) {
                                        Ok(new_tab) => {
                                            tabs.lock().unwrap().push(Arc::new(new_tab));
                                        },
                                        Err(_tab_creation_err) => {
                                            info!("Failed to create a handle to new tab");
                                            break;
                                        },
                                    }
                                }
                            },
                            Event::TargetInfoChanged(ev) => {
                                let target_info = ev.params.target_info;
                                trace!("Target info changed: {:?}", target_info);
                                if target_info.Type == "page" {
                                    let locked_tabs = tabs.lock().unwrap();
                                    let updated_tab = locked_tabs
                                        .iter()
                                        .find(|tab| *tab.get_target_id() == target_info.target_id)
                                        .expect("got TargetInfoChanged event about a tab not in our list");
                                    updated_tab.update_target_info(target_info);
                                }
                            },
                            Event::TargetDestroyed(ev) => {
                                trace!("Target destroyed: {:?}", ev.params.target_id);
                                let mut locked_tabs = tabs.lock().unwrap();
                                let pos = locked_tabs
                                    .iter()
                                    .position(|tab| *tab.get_target_id() == ev.params.target_id);

                                if let Some(idx) = pos {
                                    locked_tabs.remove(idx);
                                }
                            },
                            _ => {
                                let raw_event = format!("{event:?}");
                                trace!(
                                    "Unhandled event: {}",
                                    raw_event.chars().take(50).collect::<String>()
                                );
                            },
                        }
                    },
                }
            }
            info!("Finished browser's event handling loop");
        }).await;
    }

    /// Call a browser method.
    ///
    /// See the `cdtp` module documentation for available methods.
    async fn call_method<C>(&self, method: C) -> Result<C::ReturnObject, ChromeError>
    where
        C: Method + serde::Serialize,
    {
        self.inner.transport.call_method_on_browser(method).await
    }

    #[allow(dead_code)]
    #[cfg(test)]
    pub(crate) fn process(&self) -> Option<&Process> {
        #[allow(clippy::used_underscore_binding)]
        self.inner.process.as_ref()
    }
}

/// [`Browser`] is being dropped!
/// Dropping the inner browser means that there are no more references in the `Arc` inside [`Browser`].
impl Drop for BrowserInner {
    fn drop(&mut self) {
        info!("Dropping browser");
        let _ = self.loop_shutdown_tx.send(());
        self.transport.shutdown();
    }
}

/// Returns the path to Chrome's executable.
///
/// If the `CHROME` environment variable is set, `default_executable` will
/// use it as the default path. Otherwise, the filenames `google-chrome-stable`
/// `chromium`, `chromium-browser`, `chrome` and `chrome-browser` are
/// searched for in standard places. If that fails,
/// `/Applications/Google Chrome.app/...` (on MacOS) or the registry (on Windows)
/// is consulted. If all of the above fail, an error is returned.
pub async fn default_executable() -> Result<std::path::PathBuf, ChromeError> {
    if let Ok(path) = std::env::var("CHROME") {
        if std::path::Path::new(&path).exists() {
            return Ok(path.into());
        }
    }

    for app in [
        "google-chrome-stable",
        "google-chrome-beta",
        "google-chrome-dev",
        "google-chrome-unstable",
        "chromium",
        "chromium-browser",
        "microsoft-edge-stable",
        "microsoft-edge-beta",
        "microsoft-edge-dev",
        "chrome",
        "chrome-browser",
        "msedge",
        "microsoft-edge",
    ] {
        if let Ok(path) = which(app) {
            return Ok(path);
        }
        tokio::task::yield_now().await;
    }

    #[cfg(target_os = "macos")]
    {
        for path in [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Google Chrome Beta.app/Contents/MacOS/Google Chrome Beta",
            "/Applications/Google Chrome Dev.app/Contents/MacOS/Google Chrome Dev",
            "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            "/Applications/Microsoft Edge Beta.app/Contents/MacOS/Microsoft Edge Beta",
            "/Applications/Microsoft Edge Dev.app/Contents/MacOS/Microsoft Edge Dev",
            "/Applications/Microsoft Edge Canary.app/Contents/MacOS/Microsoft Edge Canary",
        ] {
            let path = path.into();
            if let Ok(true) = tokio::fs::try_exists(path).await {
                return Ok(path);
            }
        }
    }

    #[cfg(windows)]
    {
        use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};

        let chrome_path_from_registry = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\chrome.exe")
            .and_then(|key| key.get_value::<String, _>(""))
            .map(std::path::PathBuf::from)
            .ok();

        if let Some(path) = chrome_path_from_registry() {
            if let Ok(true) = tokio::fs::try_exists(path).await {
                return Ok(path);
            } else {
                for path in [r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"] {
                    let path = path.into();
                    if let Ok(true) = tokio::fs::try_exists(path).await {
                        return Ok(path);
                    }
                }
            }
        }
    }

    Err(ChromeError::NoExecutable)
}

#[cfg(test)]
mod test {
    use super::Browser;

    fn is_sync<T>()
    where
        T: Sync,
    {
    }

    #[test]
    fn test_if_browser_is_sync() {
        is_sync::<Browser>();
    }
}
