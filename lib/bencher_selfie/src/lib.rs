use std::time::Duration;

use headless_chrome::protocol::cdp::Page;
use headless_chrome::{Browser, LaunchOptionsBuilder};

mod error;

pub use error::{HeadlessChromeError, SelfieError};

use crate::error::map_err;

const DEFAULT_WORDMARK_SELECTOR: &str = "#wordmark";
const DEFAULT_PERF_SELECTOR: &str = "#perf";

pub struct Selfie {
    browser: Browser,
    timeout: Option<Duration>,
}

impl Selfie {
    pub fn new_embedded(timeout: Option<u64>) -> Result<Self, SelfieError> {
        Self::new(1200, 800, timeout)
    }

    pub fn new(width: u32, height: u32, timeout: Option<u64>) -> Result<Self, SelfieError> {
        let window_size = Some((width, height));
        let launch_options = map_err!(LaunchOptionsBuilder::default()
            .window_size(window_size)
            .build())?;
        let browser = map_err!(Browser::new(launch_options))?;
        let timeout = timeout.map(Duration::from_secs);
        Ok(Self { browser, timeout })
    }

    pub fn capture_perf(&self, url: &str) -> Result<Vec<u8>, SelfieError> {
        self.capture(
            url,
            &[
                (DEFAULT_WORDMARK_SELECTOR, Some(10)),
                (DEFAULT_PERF_SELECTOR, Some(5)),
            ],
        )
    }

    pub fn capture(
        &self,
        url: &str,
        selectors: &[(&str, Option<u64>)],
    ) -> Result<Vec<u8>, SelfieError> {
        let tab = map_err!(self.browser.new_tab())?;
        if let Some(timeout) = self.timeout {
            tab.set_default_timeout(timeout);
        }

        map_err!(tab.navigate_to(url))?;

        for (selector, timeout) in selectors {
            map_err!(if let Some(timeout) = timeout.map(Duration::from_secs) {
                tab.wait_for_element_with_custom_timeout(selector, timeout)
            } else {
                tab.wait_for_element(selector)
            })?;
        }

        let jpg = map_err!(tab.capture_screenshot(
            Page::CaptureScreenshotFormatOption::Jpeg,
            None,
            None,
            true
        ))?;

        if map_err!(tab.close_with_unload())? {
            Ok(jpg)
        } else {
            Err(SelfieError::CloseTab(url.into()))
        }
    }
}

// #[cfg(feature = "browser")]
#[cfg(test)]
mod test {
    use std::{fs::File, io::Write};

    use crate::Selfie;

    const PERF_ADAPTERS_URL: &str = "http://localhost:3000/perf/the-computer?key=true&metric_kind=latency&branches=903e91fe-fc30-4695-98af-a8426e7bbcfc&tab=benchmarks&testbeds=3ec87a4d-28ff-478c-b6a2-55a06ead3984&benchmarks=ab15ac98-726c-45c9-8a4f-6b4bc121e889%2C5958c90b-8e3b-4507-89cf-e6a2e763f902";

    fn save_jpg(jpg: &[u8]) {
        let mut file = File::create("perf.jpg").unwrap();
        file.write_all(jpg).unwrap();
    }

    #[test]
    fn test_selfie() {
        let selfie = Selfie::new_embedded(None).unwrap();
        let jpg = selfie.capture_perf(PERF_ADAPTERS_URL).unwrap();
        save_jpg(&jpg);
    }
}
