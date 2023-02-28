use std::fs::File;
use std::io::Write;
use std::time::Duration;

use headless_chrome::protocol::cdp::Page;
use headless_chrome::{Browser, LaunchOptionsBuilder};

// TODO move over to actual id for selector
// const PERF_ID: &str = "svg";
const PERF_ID: &str = "#perf";

pub fn screenshot(url: &str) {
    println!("BROWSER");
    let launch_options = LaunchOptionsBuilder::default()
        .window_size(Some((1200, 800)))
        .build()
        .unwrap();
    let browser = Browser::new(launch_options).unwrap();

    println!("TAB");
    let tab = browser.new_tab().unwrap();
    let timeout = Duration::from_secs(10);
    tab.set_default_timeout(timeout);

    println!("NAVIGATE");
    // Navigate to Bencher
    tab.navigate_to(url).unwrap();

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

    // const PERF_ADAPTERS_URL: &str = "https://bencher.dev/perf/bencher?key=true&metric_kind=latency&tab=benchmarks&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&start_time=2023-01-30T00%3A00%3A00.000Z&branches=619d15ed-0fbd-4ccb-86cb-fddf3124da29&benchmarks=3525f177-fc8f-4a92-bd2f-dda7c4e15699%2C5655ed2a-3e45-4622-bdbd-39cdd9837af8%2C1db23e93-f909-40aa-bf42-838cc7ae05f5";
    const PERF_ADAPTERS_URL: &str = "http://localhost:3000/perf/the-computer?key=true&metric_kind=latency&branches=903e91fe-fc30-4695-98af-a8426e7bbcfc&tab=benchmarks&testbeds=3ec87a4d-28ff-478c-b6a2-55a06ead3984&benchmarks=ab15ac98-726c-45c9-8a4f-6b4bc121e889%2C5958c90b-8e3b-4507-89cf-e6a2e763f902";

    #[test]
    fn test_screenshot() {
        println!("Test");
        screenshot(PERF_ADAPTERS_URL);
    }
}
