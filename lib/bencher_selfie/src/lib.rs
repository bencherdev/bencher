use std::time::Duration;

use headless_chrome::protocol::cdp::Page;
use headless_chrome::{Browser, LaunchOptionsBuilder, Tab};

mod error;

pub use error::{HeadlessChromeError, SelfieError};

use crate::error::map_err;

const DEFAULT_WORDMARK_SELECTOR: &str = "#wordmark";
const DEFAULT_PERF_SELECTOR: &str = "#perf";
// TODO change list to the actual embedded ID
const DEFAULT_EMBEDDED_SELECTOR: &str = "#perf";

pub struct Selfie {
    browser: Browser,
    timeout: Option<Duration>,
}

impl Selfie {
    pub fn new_embedded() -> Result<Self, SelfieError> {
        Self::new(1200, 800, None)
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
            DEFAULT_EMBEDDED_SELECTOR,
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

// #[cfg(feature = "browser")]
// #[cfg(test)]
// mod test {
//     use std::{fs::File, io::Write};

//     use crate::Selfie;

//     const PERF_ADAPTERS_URL: &str = "http://localhost:3000/perf/the-computer?key=true&metric_kind=latency&branches=903e91fe-fc30-4695-98af-a8426e7bbcfc&tab=benchmarks&testbeds=3ec87a4d-28ff-478c-b6a2-55a06ead3984&benchmarks=ab15ac98-726c-45c9-8a4f-6b4bc121e889%2C5958c90b-8e3b-4507-89cf-e6a2e763f902";

//     fn save_jpg(jpg: &[u8]) {
//         let mut file = File::create("perf.jpg").unwrap();
//         file.write_all(jpg).unwrap();
//     }

//     #[test]
//     fn test_selfie() {
//         let selfie = Selfie::new_embedded().unwrap();
//         let jpg = selfie.capture_perf(PERF_ADAPTERS_URL).unwrap();
//         save_jpg(&jpg);
//     }
// }
