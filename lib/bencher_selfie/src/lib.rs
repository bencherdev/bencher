use std::fs::File;
use std::io::Write;
use std::time::Duration;

use headless_chrome::protocol::cdp::Page;
use headless_chrome::Browser;

// TODO move over to actual id for selector
// const PERF_ID: &str = "#perf";
const PERF_ID: &str = "svg";

pub fn screenshot() {
    println!("BROWSER");
    let browser = Browser::default().unwrap();

    println!("TAB");
    let tab = browser.new_tab().unwrap();
    let timeout = Duration::from_secs(10);
    tab.set_default_timeout(timeout);

    println!("NAVIGATE");
    // Navigate to Bencher
    tab.navigate_to("https://bencher.dev/perf/bencher?key=true&metric_kind=latency&tab=benchmarks&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&start_time=2023-01-30T00%3A00%3A00.000Z&branches=619d15ed-0fbd-4ccb-86cb-fddf3124da29&benchmarks=3525f177-fc8f-4a92-bd2f-dda7c4e15699%2C5655ed2a-3e45-4622-bdbd-39cdd9837af8%2C1db23e93-f909-40aa-bf42-838cc7ae05f5").unwrap();

    println!("ELEMENT");
    // Wait for perf plot element to load
    let _perf_element = tab.wait_for_element(PERF_ID).unwrap();

    println!("JPG");
    // Take a screenshot of the entire browser window
    let jpeg_data = tab
        .capture_screenshot(Page::CaptureScreenshotFormatOption::Jpeg, None, None, true)
        .unwrap();

    println!("FILE");
    let mut file = File::create("perf.jpg").unwrap();
    file.write_all(&jpeg_data).unwrap();
}

#[cfg(feature = "browser")]
#[cfg(test)]
mod test {
    use super::screenshot;

    #[test]
    fn test_screenshot() {
        screenshot();
    }
}
