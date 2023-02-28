use std::fs::File;
use std::io::Write;
use std::time::Duration;

use headless_chrome::protocol::cdp::Page;
use headless_chrome::{Browser, LaunchOptions};

const PERF_ID: &str = "#perf";

pub fn screenshot() {
    let browser = Browser::default().unwrap();

    let tab = browser.new_tab().unwrap();

    // Navigate to Bencher
    tab.navigate_to("http://localhost:3000/perf/the-computer?key=true&branches=d73c01e5-c54b-4481-b2bf-3191c791caa7&tab=benchmarks&testbeds=6ea3ec62-d2c2-4ac1-853a-47e74c0e63c7&benchmarks=25e3855f-f760-4539-ab85-ed9571bfd3cd%2C22b18f5f-faa9-4e0c-8559-f50b37824d92&metric_kind=latency").unwrap();

    let timeout = Duration::from_secs(15);
    // Wait for perf plot element to load
    let _perf_element = tab
        .wait_for_element_with_custom_timeout(PERF_ID, timeout)
        .unwrap();

    // Take a screenshot of the entire browser window
    let jpeg_data = tab
        .capture_screenshot(Page::CaptureScreenshotFormatOption::Jpeg, None, None, true)
        .unwrap();

    let mut file = File::create("perf.jpg").unwrap();
    file.write_all(&jpeg_data).unwrap();
}

#[cfg(test)]
mod test {
    use super::screenshot;

    #[test]
    // #[ignore]
    fn test_screenshot() {
        screenshot();
    }
}
