use std::time::Duration;

use headless_chrome::protocol::cdp::Page;
use headless_chrome::{Browser, LaunchOptionsBuilder, Tab};

mod error;

pub use error::{HeadlessChromeError, SelfieError};

use crate::error::map_err;

const DEFAULT_WORDMARK_SELECTOR: &str = "#perf_wordmark";
const DEFAULT_PLOT_SELECTOR: &str = "#perf_plot";
const DEFAULT_KEY_SELECTOR: &str = "#perf_key";
const DEFAULT_VIEWPORT_SELECTOR: &str = "#perf_viewport";

pub struct Selfie {
    browser: Browser,
    timeout: Option<Duration>,
}

impl Selfie {
    pub fn new_embedded() -> Result<Self, SelfieError> {
        Self::new(1200, 1200, None)
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
                (DEFAULT_WORDMARK_SELECTOR, Some(20)),
                (DEFAULT_PLOT_SELECTOR, Some(10)),
                (DEFAULT_KEY_SELECTOR, Some(10)),
            ],
            DEFAULT_VIEWPORT_SELECTOR,
            Some(1),
        )
    }

    pub fn capture(
        &self,
        url: &str,
        wait_for: &[(&str, Option<u64>)],
        viewport: &str,
        timeout: Option<u64>,
    ) -> Result<Vec<u8>, SelfieError> {
        let tab = map_err!(self.browser.new_tab())?;

        let jpg = self.capture_inner(&tab, url, wait_for, viewport, timeout);

        // Always try to close the tab
        if map_err!(tab.close_with_unload())? {
            jpg
        } else if let Err(e) = jpg {
            Err(e)
        } else {
            Err(SelfieError::CloseTab(url.into()))
        }
    }

    fn capture_inner(
        &self,
        tab: &Tab,
        url: &str,
        wait_for: &[(&str, Option<u64>)],
        viewport: &str,
        timeout: Option<u64>,
    ) -> Result<Vec<u8>, SelfieError> {
        if let Some(timeout) = self.timeout {
            tab.set_default_timeout(timeout);
        }

        map_err!(tab.navigate_to(url))?;

        for (selector, timeout) in wait_for {
            map_err!(if let Some(timeout) = timeout.map(Duration::from_secs) {
                tab.wait_for_element_with_custom_timeout(selector, timeout)
            } else {
                tab.wait_for_element(selector)
            })?;
        }

        let element = map_err!(if let Some(timeout) = timeout.map(Duration::from_secs) {
            tab.wait_for_element_with_custom_timeout(viewport, timeout)
        } else {
            tab.wait_for_element(viewport)
        })?;
        let box_model = map_err!(element.get_box_model())?;
        let viewport = Some(box_model.margin_viewport());

        map_err!(tab.capture_screenshot(
            Page::CaptureScreenshotFormatOption::Jpeg,
            None,
            viewport,
            true
        ))
    }
}

// TODO reenable once in production
#[cfg(not(feature = "browser"))]
#[cfg(test)]
mod test {
    use std::{fs::File, io::Write};

    use crate::Selfie;

    const PERF_ADAPTERS_URL: &str = "https://bencher.dev/perf/bencher?key=true&metric_kind=latency&tab=benchmarks&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&branches=619d15ed-0fbd-4ccb-86cb-fddf3124da29&benchmarks=3525f177-fc8f-4a92-bd2f-dda7c4e15699%2C5655ed2a-3e45-4622-bdbd-39cdd9837af8%2C1db23e93-f909-40aa-bf42-838cc7ae05f5&start_time=1674777600000";

    fn save_jpeg(jpeg: &[u8]) {
        let mut file = File::create("perf.jpeg").unwrap();
        file.write_all(jpeg).unwrap();
    }

    #[test]
    fn test_selfie() {
        let selfie = Selfie::new_embedded().unwrap();
        let jpeg = selfie.capture_perf(PERF_ADAPTERS_URL).unwrap();
        save_jpeg(&jpeg);
    }
}
