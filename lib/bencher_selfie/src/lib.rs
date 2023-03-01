use std::fs::File;
use std::io::Write;
use std::time::Duration;

use headless_chrome::protocol::cdp::Page;
use headless_chrome::{Browser, LaunchOptionsBuilder};

mod error;

pub use error::{HeadlessChromeError, SelfieError};

use crate::error::map_err;

const DEFAULT_WINDOW_SIZE: (u32, u32) = (1200, 800);
const DEFAULT_TIMEOUT: u64 = 10;

pub fn screenshot(
    url: &str,
    window_size: Option<(u32, u32)>,
    selectors: &[&str],
    timeout: Option<u64>,
) -> Result<(), SelfieError> {
    println!("BROWSER");
    let window_size = window_size.or(Some(DEFAULT_WINDOW_SIZE));
    let launch_options = map_err!(LaunchOptionsBuilder::default()
        .window_size(window_size)
        .build())?;
    let browser = map_err!(Browser::new(launch_options))?;

    println!("TAB");
    let tab = map_err!(browser.new_tab())?;
    let timeout = Duration::from_secs(timeout.unwrap_or(DEFAULT_TIMEOUT));
    tab.set_default_timeout(timeout);

    println!("NAVIGATE");
    // Navigate to URL
    map_err!(tab.navigate_to(url))?;

    println!("ELEMENT");
    // Wait for element to load
    for selector in selectors {
        let _element = map_err!(tab.wait_for_element(selector))?;
    }

    println!("JPG");
    // Take a screenshot of the entire browser window
    let jpeg_data = map_err!(tab.capture_screenshot(
        Page::CaptureScreenshotFormatOption::Jpeg,
        None,
        None,
        true
    ))?;

    println!("FILE");
    let mut file = File::create("perf.jpg").unwrap();
    file.write_all(&jpeg_data).unwrap();

    Ok(())
}

// #[cfg(feature = "browser")]
#[cfg(test)]
mod test {
    use super::screenshot;

    // const PERF_ADAPTERS_URL: &str = "https://bencher.dev/perf/bencher?key=true&metric_kind=latency&tab=benchmarks&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&start_time=2023-01-30T00%3A00%3A00.000Z&branches=619d15ed-0fbd-4ccb-86cb-fddf3124da29&benchmarks=3525f177-fc8f-4a92-bd2f-dda7c4e15699%2C5655ed2a-3e45-4622-bdbd-39cdd9837af8%2C1db23e93-f909-40aa-bf42-838cc7ae05f5";
    // const DEFAULT_WORDMARK_SELECTOR: &str = "img";
    // const DEFAULT_SELECTOR: &str = "svg";

    const PERF_ADAPTERS_URL: &str = "http://localhost:3000/perf/the-computer?key=true&metric_kind=latency&branches=903e91fe-fc30-4695-98af-a8426e7bbcfc&tab=benchmarks&testbeds=3ec87a4d-28ff-478c-b6a2-55a06ead3984&benchmarks=ab15ac98-726c-45c9-8a4f-6b4bc121e889%2C5958c90b-8e3b-4507-89cf-e6a2e763f902";
    const DEFAULT_WORDMARK_SELECTOR: &str = "#wordmark";
    const DEFAULT_PERF_SELECTOR: &str = "#perf";

    #[test]
    fn test_screenshot() {
        println!("Test");
        screenshot(
            PERF_ADAPTERS_URL,
            None,
            &[DEFAULT_WORDMARK_SELECTOR, DEFAULT_PERF_SELECTOR],
            None,
        )
        .unwrap();
    }
}
