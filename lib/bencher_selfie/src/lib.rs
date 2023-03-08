use std::time::Duration;

pub use bencher_chrome::ChromeError;
use bencher_chrome::{protocol::cdp::Page, Browser, LaunchOptionsBuilder, Tab};

mod error;

pub use error::SelfieError;

const DEFAULT_WORDMARK_SELECTOR: &str = "#perf_wordmark";
const DEFAULT_PLOT_SELECTOR: &str = "#perf_plot";
const DEFAULT_KEY_SELECTOR: &str = "#perf_key";
const DEFAULT_VIEWPORT_SELECTOR: &str = "#perf_viewport";

pub struct Selfie {
    browser: Browser,
    timeout: Option<Duration>,
}

impl Selfie {
    pub async fn new_embedded() -> Result<Self, SelfieError> {
        Self::new(1200, 1200, None).await
    }

    pub async fn new(width: u32, height: u32, timeout: Option<u64>) -> Result<Self, SelfieError> {
        let window_size = Some((width, height));
        let launch_options = LaunchOptionsBuilder::default()
            .sandbox(false)
            .port(Some(8118))
            .window_size(window_size)
            .build()?;
        let browser = Browser::new(launch_options)?;
        let timeout = timeout.map(Duration::from_secs);
        Ok(Self { browser, timeout })
    }

    pub async fn capture_perf(&self, url: &str) -> Result<Vec<u8>, SelfieError> {
        self.capture(
            url,
            &[
                (DEFAULT_WORDMARK_SELECTOR, Some(10)),
                (DEFAULT_PLOT_SELECTOR, Some(10)),
                (DEFAULT_KEY_SELECTOR, Some(10)),
            ],
            DEFAULT_VIEWPORT_SELECTOR,
            Some(1),
        )
        .await
    }

    pub async fn capture(
        &self,
        url: &str,
        wait_for: &[(&str, Option<u64>)],
        viewport: &str,
        timeout: Option<u64>,
    ) -> Result<Vec<u8>, SelfieError> {
        let tab = self.browser.new_tab().await?;

        let jpeg = self
            .capture_inner(&tab, url, wait_for, viewport, timeout)
            .await;

        // Always try to close the tab
        if tab.close_with_unload()? {
            jpeg
        } else if let Err(e) = jpeg {
            Err(e)
        } else {
            Err(SelfieError::CloseTab(url.into()))
        }
    }

    async fn capture_inner(
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

        tab.navigate_to(url)?;

        // This gives the runtime a chance to poll the other tasks that need to run while things load
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        for (selector, timeout) in wait_for {
            // This signals to the runtime to poll the other tasks that still need to run
            tokio::task::yield_now().await;

            if let Some(timeout) = timeout.map(Duration::from_secs) {
                tab.wait_for_element_with_custom_timeout(selector, timeout)?
            } else {
                tab.wait_for_element(selector)?
            }
        }

        let element = if let Some(timeout) = timeout.map(Duration::from_secs) {
            tab.wait_for_element_with_custom_timeout(viewport, timeout)?
        } else {
            tab.wait_for_element(viewport)?
        };
        let box_model = element.get_box_model()?;
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
#[cfg(feature = "browser")]
#[cfg(test)]
mod test {
    use std::{fs::File, io::Write};

    use crate::Selfie;

    const PERF_ADAPTERS_URL: &str = "https://bencher.dev/perf/bencher?img=true&key=true&metric_kind=latency&tab=benchmarks&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&branches=619d15ed-0fbd-4ccb-86cb-fddf3124da29&benchmarks=3525f177-fc8f-4a92-bd2f-dda7c4e15699%2C5655ed2a-3e45-4622-bdbd-39cdd9837af8%2C1db23e93-f909-40aa-bf42-838cc7ae05f5&start_time=1674777600000";

    fn save_jpeg(jpeg: &[u8]) {
        let mut file = File::create("perf.jpeg").unwrap();
        file.write_all(jpeg).unwrap();
    }

    #[tokio::test]
    async fn test_selfie() {
        let selfie = Selfie::new_embedded().await.unwrap();
        let jpeg = selfie.capture_perf(PERF_ADAPTERS_URL).await.unwrap();
        save_jpeg(&jpeg);
    }
}
